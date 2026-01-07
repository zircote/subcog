//! MCP server setup and lifecycle.
//!
//! Implements an rmcp-based MCP server over stdio or HTTP transport.
//!
//! ## Transport Authentication
//!
//! - **Stdio**: No authentication required (trusted local process).
//! - **HTTP**: JWT bearer token authentication required (SEC-H1).
//!   Requires `http` feature and `SUBCOG_MCP_JWT_SECRET` environment variable.

use crate::mcp::{
    ResourceContent, ResourceDefinition, ResourceHandler, ToolContent, ToolDefinition, ToolRegistry,
    ToolResult,
};
use crate::models::{EventMeta, MemoryEvent};
use crate::observability::{
    RequestContext as ObsRequestContext, current_request_id, flush_metrics, scope_request_context,
};
use crate::security::record_event;
use crate::services::ServiceContainer;
use crate::{Error, Result as SubcogResult};
#[cfg(feature = "http")]
use axum::extract::{Request, State};
#[cfg(feature = "http")]
use axum::http::{Method, StatusCode, header};
#[cfg(feature = "http")]
use axum::middleware::Next;
#[cfg(feature = "http")]
use axum::response::{IntoResponse, Response};
#[cfg(feature = "http")]
use axum::routing::any_service;
#[cfg(feature = "http")]
use axum::{Json, Router};
use rmcp::model::{
    AnnotateAble, CallToolRequestParam, CallToolResult, Content, GetPromptRequestParam,
    GetPromptResult, Implementation, ListPromptsResult, ListResourceTemplatesResult,
    ListResourcesResult, ListToolsResult, PaginatedRequestParam, RawResource, Resource,
    ResourceContents, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::stdio;
#[cfg(feature = "http")]
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
#[cfg(feature = "http")]
use rmcp::transport::streamable_http_server::tower::{
    StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::borrow::Cow;
#[cfg(feature = "http")]
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
#[cfg(feature = "http")]
use tower_http::cors::CorsLayer;
#[cfg(feature = "http")]
use tower_http::set_header::SetResponseHeaderLayer;
#[cfg(feature = "http")]
use tower_http::trace::TraceLayer;
use tracing::{Instrument, info_span};

type McpResult<T> = std::result::Result<T, McpError>;

fn record_mcp_metrics<T>(operation: &'static str, start: Instant, result: &McpResult<T>) {
    let status = if result.is_ok() { "success" } else { "error" };
    metrics::counter!(
        "mcp_requests_total",
        "operation" => operation,
        "status" => status
    )
    .increment(1);
    if result.is_err() {
        metrics::counter!("mcp_request_errors_total", "operation" => operation).increment(1);
    }
    metrics::histogram!("mcp_request_duration_ms", "operation" => operation)
        .record(start.elapsed().as_secs_f64() * 1000.0);
}

async fn run_mcp_with_context<T, F, Fut>(
    request_context: Option<ObsRequestContext>,
    span: tracing::Span,
    operation: &'static str,
    f: F,
) -> McpResult<T>
where
    F: FnOnce(Instant) -> Fut,
    Fut: std::future::Future<Output = McpResult<T>>,
{
    let run = async move {
        let _span_guard = span.enter();
        let start = Instant::now();
        let result = f(start).await;
        record_mcp_metrics(operation, start, &result);
        result
    };

    if let Some(context) = request_context {
        scope_request_context(context, run).await
    } else {
        run.await
    }
}

fn init_request_context(
    existing_request_id: Option<String>,
) -> (Option<ObsRequestContext>, String) {
    if let Some(request_id) = existing_request_id {
        (None, request_id)
    } else {
        let context = ObsRequestContext::new();
        let request_id = context.request_id().to_string();
        (Some(context), request_id)
    }
}

async fn await_shutdown(cancel_token: rmcp::service::RunningServiceCancellationToken) {
    while !is_shutdown_requested() {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    cancel_token.cancel();
}

fn execute_call_tool(
    state: &McpState,
    request: CallToolRequestParam,
    context: &RequestContext<RoleServer>,
    start: Instant,
) -> McpResult<CallToolResult> {
    #[cfg(feature = "http")]
    if let Err(err) = ensure_tool_authorized(&state.tool_auth, context, &request.name) {
        record_event(MemoryEvent::McpRequestError {
            meta: EventMeta::new("mcp", current_request_id()),
            operation: "call_tool".to_string(),
            error: err.to_string(),
        });
        return Err(err);
    }
    #[cfg(not(feature = "http"))]
    let _ = context;

    let arguments = match request.arguments {
        Some(args) => Value::Object(args),
        None => Value::Object(Map::new()),
    };

    let result = match state.tools.execute(&request.name, arguments) {
        Ok(result) => result,
        Err(err) => {
            record_event(MemoryEvent::McpRequestError {
                meta: EventMeta::new("mcp", current_request_id()),
                operation: "call_tool".to_string(),
                error: err.to_string(),
            });
            return Err(McpError::invalid_params(err.to_string(), None));
        },
    };

    let status = if result.is_error { "error" } else { "success" };
    let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    record_event(MemoryEvent::McpToolExecuted {
        meta: EventMeta::new("mcp", current_request_id()),
        tool_name: request.name.to_string(),
        status: status.to_string(),
        duration_ms,
        error: result
            .is_error
            .then_some("tool execution returned error".to_string()),
    });

    Ok(tool_result_to_rmcp(result))
}

/// Global shutdown flag for graceful termination (RES-M4).
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Checks if shutdown has been requested.
#[must_use]
pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

/// Requests a graceful shutdown.
pub fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

/// Sets up the signal handler for graceful shutdown (RES-M4).
///
/// Installs a handler for SIGINT (Ctrl+C) and SIGTERM that:
/// 1. Sets the shutdown flag
/// 2. Logs the shutdown request
/// 3. Flushes metrics
///
/// # Errors
///
/// Returns an error if the signal handler cannot be installed.
pub fn setup_signal_handler() -> SubcogResult<()> {
    ctrlc::set_handler(move || {
        tracing::info!("Shutdown signal received, initiating graceful shutdown");
        request_shutdown();

        // Flush metrics immediately
        flush_metrics();

        metrics::counter!("mcp_shutdown_signals_total").increment(1);
    })
    .map_err(|e| Error::OperationFailed {
        operation: "setup_signal_handler".to_string(),
        cause: e.to_string(),
    })?;

    tracing::debug!("Signal handler installed for graceful shutdown");
    Ok(())
}

#[cfg(feature = "http")]
use crate::mcp::auth::{Claims, JwtAuthenticator, JwtConfig, ToolAuthorization};

/// Default maximum requests per rate limit window.
const DEFAULT_RATE_LIMIT_MAX_REQUESTS: usize = 1000;

/// Default rate limit window duration (1 minute).
const DEFAULT_RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Default allowed CORS origin (none by default for security).
#[cfg(feature = "http")]
const DEFAULT_CORS_ALLOWED_ORIGIN: &str = "";

/// CORS configuration (HIGH-SEC-006).
#[cfg(feature = "http")]
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// Allowed origins (comma-separated).
    pub allowed_origins: Vec<String>,
    /// Allow credentials (cookies, auth headers).
    pub allow_credentials: bool,
    /// Max age for preflight cache (seconds).
    pub max_age_secs: u64,
}

#[cfg(feature = "http")]
impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: Vec::new(), // Deny all by default
            allow_credentials: false,
            max_age_secs: 3600,
        }
    }
}

#[cfg(feature = "http")]
#[derive(Clone)]
struct RateLimitEntry {
    count: usize,
    window_start: Instant,
}

#[cfg(feature = "http")]
#[derive(Clone)]
struct HttpAuthState {
    authenticator: JwtAuthenticator,
    rate_limit: RateLimitConfig,
    rate_limits: Arc<Mutex<HashMap<String, RateLimitEntry>>>,
}

#[cfg(feature = "http")]
async fn auth_middleware(
    State(state): State<HttpAuthState>,
    mut req: Request,
    next: Next,
) -> Response {
    let request_context = ObsRequestContext::new();
    let request_id = request_context.request_id().to_string();
    scope_request_context(request_context, async move {
        let span = info_span!(
            "subcog.mcp.auth",
            request_id = %request_id,
            component = "mcp",
            operation = "auth"
        );
        let _span_guard = span.enter();

        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok());

        let claims = if let Some(header_value) = auth_header {
            match state.authenticator.validate_header(header_value) {
                Ok(claims) => claims,
                Err(e) => {
                    tracing::warn!(error = %e, "JWT authentication failed");
                    record_event(MemoryEvent::McpAuthFailed {
                        meta: EventMeta::new("mcp", current_request_id()),
                        client_id: None,
                        reason: e.to_string(),
                    });
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({
                            "error": {
                                "code": -32000,
                                "message": format!("Authentication failed: {e}")
                            }
                        })),
                    )
                        .into_response();
                },
            }
        } else {
            record_event(MemoryEvent::McpAuthFailed {
                meta: EventMeta::new("mcp", current_request_id()),
                client_id: None,
                reason: "missing authorization header".to_string(),
            });
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": {
                        "code": -32000,
                        "message": "Authentication required"
                    }
                })),
            )
                .into_response();
        };

        let client_id = claims.sub.clone();
        let mut rate_limits = state.rate_limits.lock().await;
        let entry = rate_limits
            .entry(client_id.clone())
            .or_insert_with(|| RateLimitEntry {
                count: 0,
                window_start: Instant::now(),
            });

        if entry.window_start.elapsed() > state.rate_limit.window {
            entry.count = 0;
            entry.window_start = Instant::now();
        }

        if entry.count >= state.rate_limit.max_requests {
            tracing::warn!(
                client = %client_id,
                requests = entry.count,
                "Per-client rate limit exceeded"
            );
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": {
                        "code": -32000,
                        "message": format!(
                            "Rate limit exceeded: max {} requests per {:?}",
                            state.rate_limit.max_requests,
                            state.rate_limit.window
                        )
                    }
                })),
            )
                .into_response();
        }

        entry.count += 1;
        drop(rate_limits);

        req.extensions_mut().insert(claims);

        next.run(req).await
    })
    .await
}

#[cfg(feature = "http")]
async fn map_notification_status(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    if response.status() == StatusCode::ACCEPTED {
        *response.status_mut() = StatusCode::NO_CONTENT;
    }
    response
}

#[cfg(feature = "http")]
fn build_cors_layer(config: &CorsConfig) -> SubcogResult<CorsLayer> {
    if config.allowed_origins.is_empty() {
        return Ok(CorsLayer::new());
    }

    let mut cors = CorsLayer::new().allow_methods([
        Method::GET,
        Method::POST,
        Method::DELETE,
        Method::OPTIONS,
    ]);

    for origin in &config.allowed_origins {
        let header_value =
            origin
                .parse::<header::HeaderValue>()
                .map_err(|e| Error::OperationFailed {
                    operation: "cors_origin".to_string(),
                    cause: e.to_string(),
                })?;
        cors = cors.allow_origin(header_value);
    }

    if config.allow_credentials {
        cors = cors.allow_credentials(true);
    }

    Ok(cors.max_age(Duration::from_secs(config.max_age_secs)))
}

#[cfg(feature = "http")]
fn ensure_tool_authorized(
    tool_auth: &ToolAuthorization,
    context: &RequestContext<RoleServer>,
    tool_name: &str,
) -> McpResult<()> {
    if let Some(claims) = context.extensions.get::<Claims>()
        && !tool_auth.is_authorized(claims, tool_name)
    {
        let required_scope = tool_auth.required_scope(tool_name);
        let scope_str = required_scope.unwrap_or("unknown");
        return Err(McpError::invalid_params(
            format!("Forbidden: tool '{tool_name}' requires '{scope_str}' scope"),
            None,
        ));
    }
    Ok(())
}

#[cfg(feature = "http")]
impl CorsConfig {
    /// Creates config from environment variables.
    ///
    /// Reads `SUBCOG_MCP_CORS_ALLOWED_ORIGINS` (comma-separated list),
    /// `SUBCOG_MCP_CORS_ALLOW_CREDENTIALS`, and `SUBCOG_MCP_CORS_MAX_AGE_SECS`.
    #[must_use]
    pub fn from_env() -> Self {
        let allowed_origins = std::env::var("SUBCOG_MCP_CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| DEFAULT_CORS_ALLOWED_ORIGIN.to_string())
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        let allow_credentials = std::env::var("SUBCOG_MCP_CORS_ALLOW_CREDENTIALS")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let max_age_secs = std::env::var("SUBCOG_MCP_CORS_MAX_AGE_SECS")
            .unwrap_or_else(|_| "3600".to_string())
            .parse::<u64>()
            .unwrap_or(3600);

        Self {
            allowed_origins,
            allow_credentials,
            max_age_secs,
        }
    }

    /// Sets the allowed origins.
    #[must_use]
    pub fn with_origins(mut self, origins: Vec<String>) -> Self {
        self.allowed_origins = origins;
        self
    }

    /// Sets whether to allow credentials.
    #[must_use]
    pub const fn with_credentials(mut self, allow: bool) -> Self {
        self.allow_credentials = allow;
        self
    }
}

/// Rate limit configuration (ARCH-H1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per window.
    pub max_requests: usize,
    /// Window duration.
    pub window: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: DEFAULT_RATE_LIMIT_MAX_REQUESTS,
            window: Duration::from_secs(DEFAULT_RATE_LIMIT_WINDOW_SECS),
        }
    }
}

impl RateLimitConfig {
    /// Creates config from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        let max_requests = std::env::var("SUBCOG_MCP_RATE_LIMIT_MAX_REQUESTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT_MAX_REQUESTS);

        let window_secs = std::env::var("SUBCOG_MCP_RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT_WINDOW_SECS);

        Self {
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Sets max requests.
    #[must_use]
    pub const fn with_max_requests(mut self, max_requests: usize) -> Self {
        self.max_requests = max_requests;
        self
    }

    /// Sets window duration in seconds.
    #[must_use]
    pub const fn with_window_secs(mut self, secs: u64) -> Self {
        self.window = Duration::from_secs(secs);
        self
    }
}

/// Transport type for the MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Transport {
    /// Standard input/output (default for Claude Desktop).
    #[default]
    Stdio,
    /// HTTP transport.
    Http,
}

struct McpState {
    tools: ToolRegistry,
    resources: Mutex<ResourceHandler>,
    #[cfg(feature = "http")]
    tool_auth: ToolAuthorization,
}

#[derive(Clone)]
struct McpHandler {
    state: Arc<McpState>,
}

impl McpHandler {
    fn new(tools: ToolRegistry, resources: ResourceHandler) -> Self {
        Self {
            state: Arc::new(McpState {
                tools,
                resources: Mutex::new(resources),
                #[cfg(feature = "http")]
                tool_auth: ToolAuthorization::default(),
            }),
        }
    }
}

impl ServerHandler for McpHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: rmcp::model::ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Subcog MCP server".to_string()),
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = McpResult<ListToolsResult>> + Send + '_ {
        let state = self.state.clone();
        let (request_context, request_id) = init_request_context(current_request_id());

        async move {
            let span = info_span!(
                "subcog.mcp.list_tools",
                request_id = %request_id,
                component = "mcp",
                operation = "list_tools"
            );

            run_mcp_with_context(request_context, span, "list_tools", |_start| async move {
                let tools = state
                    .tools
                    .list_tools()
                    .into_iter()
                    .map(tool_definition_to_rmcp)
                    .collect();
                Ok(ListToolsResult::with_all_items(tools))
            })
            .await
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = McpResult<CallToolResult>> + Send + '_ {
        let state = self.state.clone();
        let (request_context, request_id) = init_request_context(current_request_id());
        let tool_name = request.name.clone();
        async move {
            let span = info_span!(
                "subcog.mcp.call_tool",
                request_id = %request_id,
                component = "mcp",
                operation = "call_tool",
                tool_name = %tool_name
            );

            run_mcp_with_context(
                request_context,
                span,
                "call_tool",
                move |start| async move { execute_call_tool(&state, request, &context, start) },
            )
            .await
        }
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = McpResult<ListResourcesResult>> + Send + '_ {
        let state = self.state.clone();
        let (request_context, request_id) = init_request_context(current_request_id());
        async move {
            let span = info_span!(
                "subcog.mcp.list_resources",
                request_id = %request_id,
                component = "mcp",
                operation = "list_resources"
            );

            run_mcp_with_context(
                request_context,
                span,
                "list_resources",
                |_start| async move {
                    let resources = state
                        .resources
                        .lock()
                        .await
                        .list_resources()
                        .into_iter()
                        .map(resource_definition_to_rmcp)
                        .collect();
                    Ok(ListResourcesResult::with_all_items(resources))
                },
            )
            .await
        }
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = McpResult<ListResourceTemplatesResult>> + Send + '_ {
        let (request_context, request_id) = init_request_context(current_request_id());

        async move {
            let span = info_span!(
                "subcog.mcp.list_resource_templates",
                request_id = %request_id,
                component = "mcp",
                operation = "list_resource_templates"
            );

            run_mcp_with_context(
                request_context,
                span,
                "list_resource_templates",
                |_start| async move { Ok(ListResourceTemplatesResult::with_all_items(Vec::new())) },
            )
            .await
        }
    }

    fn read_resource(
        &self,
        request: rmcp::model::ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = McpResult<rmcp::model::ReadResourceResult>> + Send + '_
    {
        let state = self.state.clone();
        let (request_context, request_id) = init_request_context(current_request_id());
        let resource_uri = request.uri.clone();
        async move {
            let span = info_span!(
                "subcog.mcp.read_resource",
                request_id = %request_id,
                component = "mcp",
                operation = "read_resource",
                resource_uri = %resource_uri
            );

            run_mcp_with_context(
                request_context,
                span,
                "read_resource",
                |_start| async move {
                    let content = state
                        .resources
                        .lock()
                        .await
                        .get_resource(&request.uri)
                        .map_err(|e| McpError::resource_not_found(e.to_string(), None))?;

                    let contents = vec![resource_content_to_rmcp(content)];
                    Ok(rmcp::model::ReadResourceResult { contents })
                },
            )
            .await
        }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = McpResult<ListPromptsResult>> + Send + '_ {
        let (request_context, request_id) = init_request_context(current_request_id());
        async move {
            let span = info_span!(
                "subcog.mcp.list_prompts",
                request_id = %request_id,
                component = "mcp",
                operation = "list_prompts"
            );

            run_mcp_with_context(request_context, span, "list_prompts", |_start| async move {
                Ok(ListPromptsResult::with_all_items(Vec::new()))
            })
            .await
        }
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = McpResult<GetPromptResult>> + Send + '_ {
        let (request_context, request_id) = init_request_context(current_request_id());
        let prompt_name = request.name.clone();
        async move {
            let span = info_span!(
                "subcog.mcp.get_prompt",
                request_id = %request_id,
                component = "mcp",
                operation = "get_prompt",
                prompt = %prompt_name
            );

            run_mcp_with_context(request_context, span, "get_prompt", |_start| async move {
                Err(McpError::invalid_params(
                    format!("Prompts are not supported (requested: {prompt_name})"),
                    None,
                ))
            })
            .await
        }
    }
}

fn tool_definition_to_rmcp(def: &ToolDefinition) -> Tool {
    let schema = def.input_schema.as_object().cloned().unwrap_or_default();

    Tool {
        name: Cow::Owned(def.name.clone()),
        title: None,
        description: Some(Cow::Owned(def.description.clone())),
        input_schema: Arc::new(schema),
        output_schema: None,
        annotations: None,
        icons: None,
        meta: None,
    }
}

fn tool_content_to_rmcp(content: ToolContent) -> Content {
    match content {
        ToolContent::Text { text } => Content::text(text),
        ToolContent::Image { data, mime_type } => Content::image(data, mime_type),
    }
}

fn tool_result_to_rmcp(result: ToolResult) -> CallToolResult {
    let contents = result
        .content
        .into_iter()
        .map(tool_content_to_rmcp)
        .collect();
    if result.is_error {
        CallToolResult::error(contents)
    } else {
        CallToolResult::success(contents)
    }
}

fn resource_definition_to_rmcp(def: ResourceDefinition) -> Resource {
    RawResource {
        uri: def.uri,
        name: def.name,
        title: None,
        description: def.description,
        mime_type: def.mime_type,
        size: None,
        icons: None,
        meta: None,
    }
    .no_annotation()
}

fn resource_content_to_rmcp(content: ResourceContent) -> ResourceContents {
    if let Some(text) = content.text {
        ResourceContents::TextResourceContents {
            uri: content.uri,
            mime_type: content.mime_type,
            text,
            meta: None,
        }
    } else {
        ResourceContents::BlobResourceContents {
            uri: content.uri,
            mime_type: content.mime_type,
            blob: content.blob.unwrap_or_default(),
            meta: None,
        }
    }
}

/// MCP server for subcog.
pub struct McpServer {
    /// Tool registry.
    tools: ToolRegistry,
    /// Resource handler.
    resources: ResourceHandler,
    /// Transport type.
    transport: Transport,
    /// HTTP port (if using HTTP transport).
    port: u16,
    /// Rate limit configuration (ARCH-H1).
    rate_limit: RateLimitConfig,
    /// JWT authenticator for HTTP transport (SEC-H1).
    #[cfg(feature = "http")]
    jwt_authenticator: Option<JwtAuthenticator>,
    /// CORS configuration for HTTP transport (HIGH-SEC-006).
    #[cfg(feature = "http")]
    cors_config: CorsConfig,
}

impl McpServer {
    /// Creates a new MCP server.
    #[must_use]
    pub fn new() -> Self {
        // Try to initialize RecallService for memory browsing
        let resources = Self::try_init_resources();

        Self {
            tools: ToolRegistry::new(),
            resources,
            transport: Transport::Stdio,
            port: 3000,
            rate_limit: RateLimitConfig::from_env(),
            #[cfg(feature = "http")]
            jwt_authenticator: None,
            #[cfg(feature = "http")]
            cors_config: CorsConfig::from_env(),
        }
    }

    /// Sets the CORS configuration for HTTP transport (HIGH-SEC-006).
    ///
    /// By default, no origins are allowed (deny all CORS requests).
    /// Use this to explicitly allow specific origins.
    #[cfg(feature = "http")]
    #[must_use]
    pub fn with_cors_config(mut self, config: CorsConfig) -> Self {
        self.cors_config = config;
        self
    }

    /// Sets the JWT authenticator for HTTP transport (SEC-H1).
    ///
    /// # Arguments
    ///
    /// * `authenticator` - The JWT authenticator to use for validating bearer tokens.
    #[cfg(feature = "http")]
    #[must_use]
    pub fn with_jwt_authenticator(mut self, authenticator: JwtAuthenticator) -> Self {
        self.jwt_authenticator = Some(authenticator);
        self
    }

    /// Initializes JWT authentication from environment variables.
    ///
    /// Reads `SUBCOG_MCP_JWT_SECRET`, `SUBCOG_MCP_JWT_ISSUER`, and
    /// `SUBCOG_MCP_JWT_AUDIENCE` from the environment.
    ///
    /// # Errors
    ///
    /// Returns an error if `SUBCOG_MCP_JWT_SECRET` is not set or too short.
    #[cfg(feature = "http")]
    pub fn with_jwt_from_env(self) -> SubcogResult<Self> {
        let config = JwtConfig::from_env()?;
        let authenticator = JwtAuthenticator::new(&config);
        Ok(self.with_jwt_authenticator(authenticator))
    }

    /// Sets the rate limit configuration (ARCH-H1).
    ///
    /// # Arguments
    ///
    /// * `config` - The rate limit configuration.
    #[must_use]
    pub const fn with_rate_limit(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit = config;
        self
    }

    /// Tries to initialize `ResourceHandler` with services.
    ///
    /// Uses domain-scoped index (user-level index with project facets).
    fn try_init_resources() -> ResourceHandler {
        use crate::config::SubcogConfig;
        use crate::services::PromptService;

        let mut handler = ResourceHandler::new();

        // Try to add RecallService (works in both project and user scope)
        if let Ok(services) = ServiceContainer::from_current_dir_or_user() {
            if let Ok(recall) = services.recall() {
                handler = handler.with_recall_service(recall);
            }

            // Try to add PromptService with full config (respects storage settings)
            // For user-scope, repo_path is None - PromptService still works with user storage
            if let Some(repo_path) = services.repo_path() {
                let config = SubcogConfig::load_default().with_repo_path(repo_path);
                let prompt_service =
                    PromptService::with_subcog_config(config).with_repo_path(repo_path);
                handler = handler.with_prompt_service(prompt_service);
            } else {
                // User-scope: create prompt service without repo path
                let config = SubcogConfig::load_default();
                let prompt_service = PromptService::with_subcog_config(config);
                handler = handler.with_prompt_service(prompt_service);
            }
        }

        handler
    }

    /// Sets the transport type.
    #[must_use]
    pub const fn with_transport(mut self, transport: Transport) -> Self {
        self.transport = transport;
        self
    }

    /// Sets the HTTP port.
    #[must_use]
    pub const fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Starts the MCP server with graceful shutdown handling (RES-M4).
    ///
    /// Sets up signal handlers for SIGINT/SIGTERM before starting the server.
    /// The server will gracefully shut down when a signal is received.
    ///
    /// # Errors
    ///
    /// Returns an error if the server fails to start or signal handler cannot be installed.
    pub async fn start(&mut self) -> SubcogResult<()> {
        // Set up signal handler for graceful shutdown (RES-M4)
        setup_signal_handler()?;

        let (transport, port) = match self.transport {
            Transport::Stdio => ("stdio", None),
            Transport::Http => ("http", Some(self.port)),
        };
        record_event(MemoryEvent::McpStarted {
            meta: EventMeta::new("mcp", current_request_id()),
            transport: transport.to_string(),
            port,
        });

        match self.transport {
            Transport::Stdio => self.run_stdio().await,
            Transport::Http => self.run_http().await,
        }
    }

    fn build_handler(&mut self) -> McpHandler {
        let tools = std::mem::take(&mut self.tools);
        let resources = std::mem::take(&mut self.resources);
        McpHandler::new(tools, resources)
    }

    /// Runs the server over stdio with graceful shutdown (RES-M4).
    async fn run_stdio(&mut self) -> SubcogResult<()> {
        let handler = self.build_handler();
        let service = handler
            .serve(stdio())
            .await
            .map_err(|e| Error::OperationFailed {
                operation: "serve_stdio".to_string(),
                cause: e.to_string(),
            })?;

        let cancel_token = service.cancellation_token();
        let span = tracing::Span::current();
        let request_context = current_request_id().map(ObsRequestContext::from_id);
        tokio::spawn(
            async move {
                let run = await_shutdown(cancel_token);
                if let Some(context) = request_context {
                    scope_request_context(context, run).await;
                } else {
                    run.await;
                }
            }
            .instrument(span),
        );

        service
            .waiting()
            .await
            .map_err(|e| Error::OperationFailed {
                operation: "wait_stdio".to_string(),
                cause: e.to_string(),
            })?;

        Ok(())
    }

    /// Performs graceful shutdown cleanup (RES-M4).
    #[allow(dead_code)]
    fn graceful_shutdown(&self) {
        let start = Instant::now();
        tracing::info!("Starting graceful shutdown sequence");

        // Flush any pending metrics
        flush_metrics();

        // Record shutdown metrics
        metrics::counter!("mcp_graceful_shutdown_total").increment(1);
        metrics::histogram!("mcp_shutdown_duration_ms")
            .record(start.elapsed().as_secs_f64() * 1000.0);

        tracing::info!(
            duration_ms = start.elapsed().as_millis(),
            "Graceful shutdown completed"
        );
    }

    /// Runs the server over HTTP with JWT authentication (SEC-H1).
    ///
    /// Requires the `http` feature and `SUBCOG_MCP_JWT_SECRET` environment variable.
    #[cfg(feature = "http")]
    async fn run_http(&mut self) -> SubcogResult<()> {
        // Ensure JWT authenticator is configured
        let authenticator = self.jwt_authenticator.clone().ok_or_else(|| {
            Error::OperationFailed {
                operation: "run_http".to_string(),
                cause: "JWT authenticator not configured. Set SUBCOG_MCP_JWT_SECRET or call with_jwt_authenticator()".to_string(),
            }
        })?;

        let handler = self.build_handler();
        let session_manager = Arc::new(LocalSessionManager::default());
        let streamable = StreamableHttpService::new(
            move || Ok(handler.clone()),
            session_manager,
            StreamableHttpServerConfig::default(),
        );

        let auth_state = HttpAuthState {
            authenticator,
            rate_limit: self.rate_limit.clone(),
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
        };

        // Build CORS layer
        let cors_layer = build_cors_layer(&self.cors_config)?;

        let app = Router::new()
            .route_service("/mcp", any_service(streamable))
            .layer(axum::middleware::from_fn_with_state(
                auth_state.clone(),
                auth_middleware,
            ))
            .layer(axum::middleware::from_fn(map_notification_status))
            // CORS layer (HIGH-SEC-006) - must be before other layers
            .layer(cors_layer)
            // Security headers (OWASP recommendations)
            .layer(SetResponseHeaderLayer::overriding(
                header::X_CONTENT_TYPE_OPTIONS,
                header::HeaderValue::from_static("nosniff"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::X_FRAME_OPTIONS,
                header::HeaderValue::from_static("DENY"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::CONTENT_SECURITY_POLICY,
                header::HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("no-store"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::HeaderName::from_static("x-permitted-cross-domain-policies"),
                header::HeaderValue::from_static("none"),
            ))
            .layer(TraceLayer::new_for_http());

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], self.port));
        tracing::info!(port = self.port, "Starting MCP HTTP server with JWT auth");

        let listener =
            tokio::net::TcpListener::bind(addr)
                .await
                .map_err(|e| Error::OperationFailed {
                    operation: "bind".to_string(),
                    cause: e.to_string(),
                })?;

        axum::serve(listener, app)
            .await
            .map_err(|e| Error::OperationFailed {
                operation: "serve".to_string(),
                cause: e.to_string(),
            })
    }

    /// Runs the server over HTTP (feature not enabled).
    #[cfg(not(feature = "http"))]
    async fn run_http(&self) -> SubcogResult<()> {
        Err(Error::FeatureNotEnabled("http".to_string()))
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_creation() {
        let server = McpServer::new();
        assert_eq!(server.transport, Transport::Stdio);
    }

    #[test]
    fn test_with_transport() {
        let server = McpServer::new()
            .with_transport(Transport::Http)
            .with_port(8080);
        assert_eq!(server.transport, Transport::Http);
        assert_eq!(server.port, 8080);
    }

    #[test]
    fn test_tool_definition_mapping() {
        let registry = ToolRegistry::new();
        let tool = registry.get_tool("subcog_status").unwrap();
        let rmcp_tool = tool_definition_to_rmcp(tool);
        assert_eq!(rmcp_tool.name, "subcog_status");
    }
}

#[cfg(all(test, feature = "http"))]
mod cors_tests {
    use super::*;

    #[test]
    fn test_cors_config_default() {
        let config = CorsConfig::default();
        assert!(config.allowed_origins.is_empty());
        assert!(!config.allow_credentials);
        assert_eq!(config.max_age_secs, 3600);
    }

    #[test]
    fn test_cors_config_with_origins() {
        let config = CorsConfig::default()
            .with_origins(vec!["https://example.com".to_string()])
            .with_credentials(true);

        assert_eq!(config.allowed_origins.len(), 1);
        assert_eq!(config.allowed_origins[0], "https://example.com");
        assert!(config.allow_credentials);
    }

    #[test]
    fn test_cors_config_from_env_defaults() {
        // Test that from_env() returns sensible defaults when env vars are not set
        // (assumes test environment doesn't have SUBCOG_MCP_CORS_* set)
        let config = CorsConfig::from_env();
        // Default max_age should be 3600
        assert_eq!(config.max_age_secs, 3600);
        // Default allow_credentials should be false
        assert!(!config.allow_credentials);
    }

    #[test]
    fn test_cors_origin_parsing() {
        // Test the parsing logic used in from_env
        let origins_str = "https://a.com, https://b.com, ";
        let origins: Vec<String> = origins_str
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        assert_eq!(origins.len(), 2);
        assert_eq!(origins[0], "https://a.com");
        assert_eq!(origins[1], "https://b.com");
    }

    #[test]
    fn test_mcp_server_with_cors_config() {
        let cors = CorsConfig::default().with_origins(vec!["https://trusted.com".to_string()]);

        let server = McpServer::new().with_cors_config(cors);

        assert_eq!(server.cors_config.allowed_origins.len(), 1);
        assert_eq!(server.cors_config.allowed_origins[0], "https://trusted.com");
    }
}

#[cfg(all(test, feature = "http"))]
mod auth_tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode, header},
        middleware,
        routing::get,
    };
    use chrono::Utc;
    use jsonwebtoken::{EncodingKey, Header};
    use tower::util::ServiceExt;

    const TEST_JWT_SECRET: &str = "a-very-long-secret-key-that-is-at-least-32-chars";

    fn build_app(state: HttpAuthState) -> Router {
        Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(state, auth_middleware))
    }

    fn build_state(max_requests: usize) -> HttpAuthState {
        let config = JwtConfig::new(TEST_JWT_SECRET);
        HttpAuthState {
            authenticator: JwtAuthenticator::new(&config),
            rate_limit: RateLimitConfig {
                max_requests,
                window: Duration::from_secs(60),
            },
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[allow(clippy::expect_used)]
    fn create_test_token(sub: &str) -> String {
        let exp =
            usize::try_from((Utc::now() + chrono::Duration::hours(1)).timestamp()).unwrap_or(0);
        let iat = usize::try_from(Utc::now().timestamp()).unwrap_or(0);
        let claims = Claims {
            sub: sub.to_string(),
            exp,
            iat,
            iss: None,
            aud: None,
            scopes: vec!["read".to_string()],
        };
        jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
        )
        .expect("Failed to encode test token")
    }

    #[tokio::test]
    async fn test_auth_middleware_missing_header() {
        let app = build_app(build_state(5));
        let request = Request::builder()
            .uri("/")
            .body(Body::empty())
            .expect("request");
        let response = app.oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_middleware_rate_limit_exceeded() {
        let state = build_state(1);
        let app = build_app(state);
        let token = create_test_token("client-a");
        let auth_header = format!("Bearer {token}");

        let request = Request::builder()
            .uri("/")
            .header(header::AUTHORIZATION, auth_header.clone())
            .body(Body::empty())
            .expect("request");
        let first = app.clone().oneshot(request).await.expect("response");
        assert_eq!(first.status(), StatusCode::OK);

        let request = Request::builder()
            .uri("/")
            .header(header::AUTHORIZATION, auth_header)
            .body(Body::empty())
            .expect("request");
        let second = app.oneshot(request).await.expect("response");
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
