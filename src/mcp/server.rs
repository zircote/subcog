//! MCP server setup and lifecycle.
//!
//! Implements a JSON-RPC based MCP server over stdio or HTTP transport.
//!
//! ## Transport Authentication
//!
//! - **Stdio**: No authentication required (trusted local process).
//! - **HTTP**: JWT bearer token authentication required (SEC-H1).
//!   Requires `http` feature and `SUBCOG_MCP_JWT_SECRET` environment variable.

use crate::mcp::{PromptRegistry, ResourceHandler, ToolRegistry};
use crate::observability::flush_metrics;
use crate::services::ServiceContainer;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tracing::info_span;

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
pub fn setup_signal_handler() -> Result<()> {
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
use crate::mcp::auth::{JwtAuthenticator, JwtConfig, ToolAuthorization};

/// Default maximum requests per rate limit window.
const DEFAULT_RATE_LIMIT_MAX_REQUESTS: usize = 1000;

/// Default rate limit window duration (1 minute).
const DEFAULT_RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Maximum request body size (1MB) to prevent `DoS` via large payloads (SEC-H4).
const MAX_REQUEST_BODY_SIZE: usize = 1024 * 1024;

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
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(false);

        let max_age_secs = std::env::var("SUBCOG_MCP_CORS_MAX_AGE_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600);

        Self {
            allowed_origins,
            allow_credentials,
            max_age_secs,
        }
    }

    /// Sets allowed origins.
    #[must_use]
    pub fn with_origins(mut self, origins: Vec<String>) -> Self {
        self.allowed_origins = origins;
        self
    }

    /// Allows credentials.
    #[must_use]
    pub const fn with_credentials(mut self, allow: bool) -> Self {
        self.allow_credentials = allow;
        self
    }
}

/// MCP rate limit configuration (ARCH-H1).
#[derive(Debug, Clone)]
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
    ///
    /// Reads `SUBCOG_MCP_RATE_LIMIT_MAX_REQUESTS` and
    /// `SUBCOG_MCP_RATE_LIMIT_WINDOW_SECS` from the environment.
    #[must_use]
    pub fn from_env() -> Self {
        let max_requests = std::env::var("SUBCOG_MCP_RATE_LIMIT_MAX_REQUESTS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT_MAX_REQUESTS);

        let window_secs = std::env::var("SUBCOG_MCP_RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT_WINDOW_SECS);

        Self {
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Sets maximum requests per window.
    #[must_use]
    pub const fn with_max_requests(mut self, max: usize) -> Self {
        self.max_requests = max;
        self
    }

    /// Sets window duration in seconds.
    #[must_use]
    pub const fn with_window_secs(mut self, secs: u64) -> Self {
        self.window = Duration::from_secs(secs);
        self
    }
}

/// MCP protocol version.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Server name.
const SERVER_NAME: &str = "subcog";

/// Transport type for the MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Transport {
    /// Standard input/output (default for Claude Desktop).
    #[default]
    Stdio,
    /// HTTP transport.
    Http,
}

/// MCP server for subcog.
pub struct McpServer {
    /// Tool registry.
    tools: ToolRegistry,
    /// Resource handler.
    resources: ResourceHandler,
    /// Prompt registry.
    prompts: PromptRegistry,
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
            prompts: PromptRegistry::new(),
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
    pub fn with_jwt_from_env(self) -> Result<Self> {
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
    /// Uses domain-scoped index (project-local `.subcog/index.db`).
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
    pub fn start(&mut self) -> Result<()> {
        // Set up signal handler for graceful shutdown (RES-M4)
        setup_signal_handler()?;

        match self.transport {
            Transport::Stdio => self.run_stdio(),
            Transport::Http => self.run_http(),
        }
    }

    /// Runs the server over stdio with rate limiting and graceful shutdown (RES-M4).
    fn run_stdio(&mut self) -> Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let reader = BufReader::new(stdin.lock());

        // Rate limiting state
        let mut request_count: usize = 0;
        let mut window_start = Instant::now();

        for line in reader.lines() {
            // Check for graceful shutdown signal (RES-M4)
            if is_shutdown_requested() {
                tracing::info!("Graceful shutdown in progress, stopping stdio server");
                self.graceful_shutdown();
                return Ok(());
            }

            let line = line.map_err(|e| Error::OperationFailed {
                operation: "read_stdin".to_string(),
                cause: e.to_string(),
            })?;

            if line.is_empty() {
                continue;
            }

            // Rate limiting: reset window if expired (ARCH-H1: configurable)
            if window_start.elapsed() > self.rate_limit.window {
                request_count = 0;
                window_start = Instant::now();
            }

            // Check rate limit
            if request_count >= self.rate_limit.max_requests {
                let max_requests = self.rate_limit.max_requests;
                let window = self.rate_limit.window;
                tracing::warn!("Rate limit exceeded: {request_count} requests in {window:?}",);
                metrics::counter!("mcp_rate_limit_exceeded_total").increment(1);

                // Return rate limit error
                let error_response = self.format_error(
                    None,
                    -32000,
                    &format!("Rate limit exceeded: max {max_requests} requests per {window:?}",),
                );
                writeln!(stdout, "{error_response}").map_err(|e| Error::OperationFailed {
                    operation: "write_stdout".to_string(),
                    cause: e.to_string(),
                })?;
                stdout.flush().map_err(|e| Error::OperationFailed {
                    operation: "flush_stdout".to_string(),
                    cause: e.to_string(),
                })?;
                continue;
            }

            request_count += 1;
            let response = self.handle_request(&line);

            writeln!(stdout, "{response}").map_err(|e| Error::OperationFailed {
                operation: "write_stdout".to_string(),
                cause: e.to_string(),
            })?;

            stdout.flush().map_err(|e| Error::OperationFailed {
                operation: "flush_stdout".to_string(),
                cause: e.to_string(),
            })?;

            // Flush metrics to push gateway after each request
            // This ensures metrics are captured even if process is killed
            flush_metrics();
        }

        Ok(())
    }

    /// Performs graceful shutdown cleanup (RES-M4).
    ///
    /// Called when a shutdown signal is received. Performs:
    /// 1. Final metrics flush
    /// 2. Logs shutdown completion
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
    fn run_http(&mut self) -> Result<()> {
        use axum::http::{Method, header};
        use axum::{Router, routing::post};
        use std::sync::{Arc, Mutex};
        use tower_http::cors::CorsLayer;
        use tower_http::set_header::SetResponseHeaderLayer;
        use tower_http::trace::TraceLayer;

        // Ensure JWT authenticator is configured
        let authenticator = self.jwt_authenticator.clone().ok_or_else(|| {
            Error::OperationFailed {
                operation: "run_http".to_string(),
                cause: "JWT authenticator not configured. Set SUBCOG_MCP_JWT_SECRET or call with_jwt_authenticator()".to_string(),
            }
        })?;

        // Create shared state for the server with per-client rate limiting
        let server = Arc::new(Mutex::new(McpHttpState {
            tools: std::mem::take(&mut self.tools),
            resources: std::mem::take(&mut self.resources),
            prompts: std::mem::take(&mut self.prompts),
            authenticator,
            tool_auth: ToolAuthorization::default(),
            rate_limit_config: self.rate_limit.clone(),
            rate_limits: std::collections::HashMap::new(),
        }));

        // Build CORS layer from configuration (HIGH-SEC-006)
        // By default, no origins are allowed (deny all CORS requests)
        let cors_layer = if self.cors_config.allowed_origins.is_empty() {
            // No origins configured - CORS preflight will fail (secure default)
            tracing::info!("CORS: No origins configured, all cross-origin requests denied");
            CorsLayer::new()
        } else {
            // Parse allowed origins into HeaderValues
            let origins: Vec<_> = self
                .cors_config
                .allowed_origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect();

            tracing::info!(
                origins = ?self.cors_config.allowed_origins,
                allow_credentials = self.cors_config.allow_credentials,
                "CORS: Configured with explicit origins"
            );

            let mut layer = CorsLayer::new()
                .allow_origin(origins)
                .allow_methods([Method::POST, Method::OPTIONS])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
                .max_age(std::time::Duration::from_secs(
                    self.cors_config.max_age_secs,
                ));

            if self.cors_config.allow_credentials {
                layer = layer.allow_credentials(true);
            }

            layer
        };

        // Build the router with security headers
        let app = Router::new()
            .route("/mcp", post(handle_http_request))
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
            .layer(TraceLayer::new_for_http())
            .with_state(server);

        // Create tokio runtime for the server
        let rt = tokio::runtime::Runtime::new().map_err(|e| Error::OperationFailed {
            operation: "create_runtime".to_string(),
            cause: e.to_string(),
        })?;

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], self.port));
        tracing::info!(port = self.port, "Starting MCP HTTP server with JWT auth");

        rt.block_on(async {
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
        })
    }

    /// Runs the server over HTTP (feature not enabled).
    #[cfg(not(feature = "http"))]
    fn run_http(&self) -> Result<()> {
        Err(Error::FeatureNotEnabled("http".to_string()))
    }

    /// Handles a JSON-RPC request.
    fn handle_request(&mut self, request: &str) -> String {
        // SEC-H4: Check request size before processing to prevent DoS
        if request.len() > MAX_REQUEST_BODY_SIZE {
            tracing::warn!(
                request_size = request.len(),
                max_size = MAX_REQUEST_BODY_SIZE,
                "Request exceeds maximum size limit"
            );
            return self.format_error(
                None,
                -32600,
                &format!(
                    "Request too large: {} bytes (max: {} bytes)",
                    request.len(),
                    MAX_REQUEST_BODY_SIZE
                ),
            );
        }

        let start = Instant::now();
        let transport_label = match self.transport {
            Transport::Stdio => "stdio",
            Transport::Http => "http",
        };

        let span = info_span!(
            "mcp.request",
            transport = transport_label,
            rpc.method = tracing::field::Empty,
            rpc.id = tracing::field::Empty,
            status = tracing::field::Empty
        );
        let _guard = span.enter();

        let parsed: std::result::Result<JsonRpcRequest, _> = serde_json::from_str(request);
        let mut method_label = "parse_error".to_string();
        let mut status_label = "error";

        let response = match parsed {
            Ok(req) => {
                method_label.clone_from(&req.method);
                span.record("rpc.method", method_label.as_str());
                if let Some(id) = &req.id {
                    let id_str = id.to_string();
                    span.record("rpc.id", id_str.as_str());
                }

                tracing::info!(method = %method_label, transport = transport_label, "Processing MCP request");

                let result = self.dispatch_method(&req.method, req.params);
                status_label = if result.is_ok() { "success" } else { "error" };
                span.record("status", status_label);
                self.format_response(req.id, result)
            },
            Err(e) => {
                span.record("status", "parse_error");
                self.format_error(None, -32700, &format!("Parse error: {e}"))
            },
        };

        metrics::counter!(
            "mcp_requests_total",
            "method" => method_label.clone(),
            "transport" => transport_label,
            "status" => status_label
        )
        .increment(1);
        metrics::histogram!(
            "mcp_request_duration_ms",
            "method" => method_label,
            "transport" => transport_label
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);

        response
    }

    /// Dispatches a method call using the command pattern.
    ///
    /// Uses [`McpMethod`] enum for type-safe method dispatch instead of
    /// string matching, following the Open/Closed Principle.
    fn dispatch_method(&mut self, method: &str, params: Option<Value>) -> DispatchResult {
        use super::dispatch::McpMethod;

        match McpMethod::from(method) {
            McpMethod::Initialize => self.handle_initialize(params),
            McpMethod::ListTools => self.handle_list_tools(),
            McpMethod::CallTool => self.handle_call_tool(params),
            McpMethod::ListResources => self.handle_list_resources(),
            McpMethod::ReadResource => self.handle_read_resource(params),
            McpMethod::ListPrompts => self.handle_list_prompts(),
            McpMethod::GetPrompt => self.handle_get_prompt(params),
            McpMethod::Ping => Ok(serde_json::json!({})),
            McpMethod::Unknown(name) => Err((-32601, format!("Method not found: {name}"))),
        }
    }

    /// Handles the initialize method.
    fn handle_initialize(&self, _params: Option<Value>) -> DispatchResult {
        Ok(serde_json::json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {},
                "resources": {},
                "prompts": {},
                "sampling": {}
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    /// Handles tools/list.
    fn handle_list_tools(&self) -> DispatchResult {
        let tools: Vec<Value> = self
            .tools
            .list_tools()
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema
                })
            })
            .collect();

        Ok(serde_json::json!({ "tools": tools }))
    }

    /// Handles tools/call.
    fn handle_call_tool(&self, params: Option<Value>) -> DispatchResult {
        let params = params.ok_or((-32602, "Missing params".to_string()))?;

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or((-32602, "Missing tool name".to_string()))?;
        let tool_name = name.to_string();
        let span = info_span!("mcp.tool.call", tool.name = tool_name.as_str());
        let _guard = span.enter();
        let start = Instant::now();

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let (result, status_label) = match self.tools.execute(name, arguments) {
            Ok(result) => {
                let status_label = if result.is_error { "error" } else { "success" };
                (
                    Ok(serde_json::json!({
                        "content": result.content,
                        "isError": result.is_error
                    })),
                    status_label,
                )
            },
            Err(e) => (
                Ok(serde_json::json!({
                    "content": [{ "type": "text", "text": e.to_string() }],
                    "isError": true
                })),
                "error",
            ),
        };
        metrics::counter!(
            "mcp_tool_calls_total",
            "tool" => tool_name.clone(),
            "status" => status_label
        )
        .increment(1);
        if status_label == "error" {
            metrics::counter!(
                "mcp_tool_errors_total",
                "tool" => tool_name.clone()
            )
            .increment(1);
        }
        metrics::histogram!(
            "mcp_tool_duration_ms",
            "tool" => tool_name,
            "status" => status_label
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);

        result
    }

    /// Handles resources/list.
    fn handle_list_resources(&self) -> DispatchResult {
        let resources: Vec<Value> = self
            .resources
            .list_resources()
            .iter()
            .map(|r| {
                serde_json::json!({
                    "uri": r.uri,
                    "name": r.name,
                    "description": r.description,
                    "mimeType": r.mime_type
                })
            })
            .collect();

        Ok(serde_json::json!({ "resources": resources }))
    }

    /// Handles resources/read.
    fn handle_read_resource(&mut self, params: Option<Value>) -> DispatchResult {
        let params = params.ok_or((-32602, "Missing params".to_string()))?;

        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or((-32602, "Missing resource URI".to_string()))?;

        let resource_kind = classify_resource_kind(uri);
        let span = info_span!(
            "mcp.resource.read",
            resource.uri = uri,
            resource.kind = resource_kind,
            status = tracing::field::Empty
        );
        let _guard = span.enter();
        let start = Instant::now();

        let result = match self.resources.get_resource(uri) {
            Ok(content) => Ok(serde_json::json!({
                "contents": [{
                    "uri": content.uri,
                    "mimeType": content.mime_type,
                    "text": content.text
                }]
            })),
            Err(e) => Err((-32603, e.to_string())),
        };

        let status_label = if result.is_ok() { "success" } else { "error" };
        span.record("status", status_label);
        metrics::counter!(
            "mcp_resource_reads_total",
            "resource_kind" => resource_kind,
            "status" => status_label
        )
        .increment(1);
        metrics::histogram!(
            "mcp_resource_read_duration_ms",
            "resource_kind" => resource_kind,
            "status" => status_label
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);

        result
    }

    /// Handles prompts/list.
    fn handle_list_prompts(&self) -> DispatchResult {
        let prompts: Vec<Value> = self
            .prompts
            .list_prompts()
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "description": p.description,
                    "arguments": p.arguments.iter().map(|a| {
                        serde_json::json!({
                            "name": a.name,
                            "description": a.description,
                            "required": a.required
                        })
                    }).collect::<Vec<Value>>()
                })
            })
            .collect();

        Ok(serde_json::json!({ "prompts": prompts }))
    }

    /// Handles prompts/get.
    fn handle_get_prompt(&self, params: Option<Value>) -> DispatchResult {
        let params = params.ok_or((-32602, "Missing params".to_string()))?;

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or((-32602, "Missing prompt name".to_string()))?;
        let span = info_span!("mcp.prompt.get", prompt.name = name);
        let _guard = span.enter();

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        match self.prompts.get_prompt_messages(name, &arguments) {
            Some(messages) => {
                let msgs: Vec<Value> = messages
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "role": m.role,
                            "content": m.content
                        })
                    })
                    .collect();

                Ok(serde_json::json!({ "messages": msgs }))
            },
            None => Err((-32602, format!("Unknown prompt: {name}"))),
        }
    }

    /// Formats a successful response.
    fn format_response(&self, id: Option<Value>, result: DispatchResult) -> String {
        match result {
            Ok(value) => {
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(value),
                    error: None,
                };
                serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string())
            },
            Err((code, message)) => self.format_error(id, code, &message),
        }
    }

    /// Formats an error response.
    fn format_error(&self, id: Option<Value>, code: i32, message: &str) -> String {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            }),
        };
        serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string())
    }
}

fn classify_resource_kind(uri: &str) -> &'static str {
    if uri.starts_with("subcog://memory/") {
        "memory"
    } else if uri.starts_with("subcog://project/") {
        "project"
    } else if uri.starts_with("subcog://help") {
        "help"
    } else {
        "other"
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result type for method dispatch.
type DispatchResult = std::result::Result<Value, (i32, String)>;

/// JSON-RPC request.
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    /// JSON-RPC version (required by protocol but not used in code).
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

/// JSON-RPC response.
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC error.
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

// HTTP transport implementation (SEC-H1)
#[cfg(feature = "http")]
#[allow(
    clippy::too_many_lines,
    clippy::excessive_nesting,
    clippy::significant_drop_tightening
)]
mod http_transport {
    use super::{
        DispatchResult, JsonRpcRequest, PromptRegistry, ResourceHandler, ToolRegistry, Value,
    };
    use crate::mcp::auth::{Claims, JwtAuthenticator, ToolAuthorization};
    use axum::{
        Json,
        extract::State,
        http::{HeaderMap, StatusCode},
        response::IntoResponse,
    };
    use std::sync::{Arc, Mutex};

    /// Per-client rate limit state.
    #[derive(Debug, Clone)]
    pub struct ClientRateLimit {
        /// Number of requests in the current window.
        pub request_count: usize,
        /// Start of the current rate limit window.
        pub window_start: std::time::Instant,
    }

    impl Default for ClientRateLimit {
        fn default() -> Self {
            Self {
                request_count: 0,
                window_start: std::time::Instant::now(),
            }
        }
    }

    /// Shared state for HTTP transport.
    pub struct McpHttpState {
        pub tools: ToolRegistry,
        pub resources: ResourceHandler,
        pub prompts: PromptRegistry,
        pub authenticator: JwtAuthenticator,
        /// Tool authorization for scope-based access control (CRIT-003).
        pub tool_auth: ToolAuthorization,
        /// Rate limit configuration (ARCH-H1).
        pub rate_limit_config: super::RateLimitConfig,
        /// Per-client rate limits keyed by JWT subject/issuer.
        pub rate_limits: std::collections::HashMap<String, ClientRateLimit>,
    }

    /// HTTP request handler with JWT authentication.
    pub async fn handle_http_request(
        State(state): State<Arc<Mutex<McpHttpState>>>,
        headers: HeaderMap,
        body: String,
    ) -> impl IntoResponse {
        // SEC-H4: Check request body size before processing to prevent DoS
        if body.len() > super::MAX_REQUEST_BODY_SIZE {
            tracing::warn!(
                body_size = body.len(),
                max_size = super::MAX_REQUEST_BODY_SIZE,
                "Request body exceeds maximum size limit"
            );
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32600,
                        "message": format!(
                            "Request body too large: {} bytes (max: {} bytes)",
                            body.len(),
                            super::MAX_REQUEST_BODY_SIZE
                        )
                    }
                })),
            );
        }

        // Extract and validate Authorization header
        let auth_header = match headers.get("authorization") {
            Some(h) => match h.to_str() {
                Ok(s) => s,
                Err(_) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32600,
                                "message": "Invalid Authorization header encoding"
                            }
                        })),
                    );
                },
            },
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -32000,
                            "message": "Missing Authorization header"
                        }
                    })),
                );
            },
        };

        // Validate JWT token and extract client identifier
        let Ok(mut state_guard) = state.lock() else {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": "Internal server error"
                    }
                })),
            );
        };

        let claims = match state_guard.authenticator.validate_header(auth_header) {
            Ok(claims) => claims,
            Err(e) => {
                tracing::warn!(error = %e, "JWT authentication failed");
                metrics::counter!("mcp_auth_failures_total", "transport" => "http").increment(1);
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -32000,
                            "message": format!("Authentication failed: {e}")
                        }
                    })),
                );
            },
        };

        metrics::counter!("mcp_auth_success_total", "transport" => "http").increment(1);

        // Per-client rate limiting using JWT subject as client identifier
        #[allow(clippy::redundant_clone)] // claims.sub used after entry() via client_id reference
        let client_id = claims.sub.clone();

        // Extract rate limit config before mutable borrow of rate_limits
        let rate_limit_window = state_guard.rate_limit_config.window;
        let rate_limit_max = state_guard.rate_limit_config.max_requests;

        let rate_limit = state_guard
            .rate_limits
            .entry(client_id.clone())
            .or_default();

        // Reset window if expired
        if rate_limit.window_start.elapsed() > rate_limit_window {
            rate_limit.request_count = 0;
            rate_limit.window_start = std::time::Instant::now();
        }

        // Check rate limit
        if rate_limit.request_count >= rate_limit_max {
            tracing::warn!(
                client = %client_id,
                requests = rate_limit.request_count,
                "Per-client rate limit exceeded"
            );
            #[allow(clippy::redundant_clone)]
            // metrics macro requires owned String for label value
            metrics::counter!(
                "mcp_rate_limit_exceeded_total",
                "transport" => "http",
                "client" => client_id.clone()
            )
            .increment(1);
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32000,
                        "message": format!(
                            "Rate limit exceeded: max {} requests per {:?}",
                            rate_limit_max,
                            rate_limit_window
                        )
                    }
                })),
            );
        }

        rate_limit.request_count += 1;

        // Parse JSON-RPC request
        let parsed: std::result::Result<JsonRpcRequest, _> = serde_json::from_str(&body);
        drop(state_guard);

        match parsed {
            Ok(req) => {
                let Ok(mut state_guard) = state.lock() else {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32603,
                                "message": "Internal server error"
                            }
                        })),
                    );
                };

                let result =
                    dispatch_http_method(&mut state_guard, &req.method, req.params, &claims);

                let response = match result {
                    Ok(value) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": req.id,
                        "result": value
                    }),
                    Err((code, message)) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": req.id,
                        "error": {
                            "code": code,
                            "message": message
                        }
                    }),
                };

                (StatusCode::OK, Json(response))
            },
            Err(e) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {e}")
                    }
                })),
            ),
        }
    }

    /// Dispatches a method call for HTTP transport.
    ///
    /// # Authorization (CRIT-003)
    ///
    /// Tool calls are authorized based on JWT scopes:
    /// - `read`: `subcog_recall`, `subcog_status`, `subcog_namespaces`, `prompt_list`, `prompt_get`, `prompt_run`
    /// - `write`: `subcog_capture`, `subcog_enrich`, `subcog_consolidate`, `prompt_save`, `prompt_delete`
    /// - `admin`: `subcog_sync`, `subcog_reindex`
    /// - `*`: Wildcard scope grants all permissions
    fn dispatch_http_method(
        state: &mut McpHttpState,
        method: &str,
        params: Option<Value>,
        claims: &Claims,
    ) -> DispatchResult {
        match method {
            "initialize" => Ok(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {},
                    "resources": {},
                    "prompts": {},
                    "sampling": {}
                },
                "serverInfo": {
                    "name": "subcog",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            "tools/list" => {
                let tools: Vec<Value> = state
                    .tools
                    .list_tools()
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "description": t.description,
                            "inputSchema": t.input_schema
                        })
                    })
                    .collect();
                Ok(serde_json::json!({ "tools": tools }))
            },
            "tools/call" => {
                let params = params.ok_or((-32602, "Missing params".to_string()))?;
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or((-32602, "Missing tool name".to_string()))?;

                // CRIT-003: Check authorization before executing tool
                if !state.tool_auth.is_authorized(claims, name) {
                    let required_scope = state.tool_auth.required_scope(name);
                    tracing::warn!(
                        tool = name,
                        required_scope = required_scope,
                        user_scopes = ?claims.scopes,
                        sub = %claims.sub,
                        "Tool authorization denied"
                    );
                    let scope_str = required_scope.unwrap_or("unknown");
                    metrics::counter!(
                        "mcp_tool_auth_denied_total",
                        "tool" => name.to_string(),
                        "required_scope" => scope_str.to_string()
                    )
                    .increment(1);
                    return Err((
                        -32000,
                        format!("Forbidden: tool '{name}' requires '{scope_str}' scope"),
                    ));
                }

                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                match state.tools.execute(name, arguments) {
                    Ok(result) => Ok(serde_json::json!({
                        "content": result.content,
                        "isError": result.is_error
                    })),
                    Err(e) => Ok(serde_json::json!({
                        "content": [{ "type": "text", "text": e.to_string() }],
                        "isError": true
                    })),
                }
            },
            "resources/list" => {
                let resources: Vec<Value> = state
                    .resources
                    .list_resources()
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "uri": r.uri,
                            "name": r.name,
                            "description": r.description,
                            "mimeType": r.mime_type
                        })
                    })
                    .collect();
                Ok(serde_json::json!({ "resources": resources }))
            },
            "resources/read" => {
                let params = params.ok_or((-32602, "Missing params".to_string()))?;
                let uri = params
                    .get("uri")
                    .and_then(|v| v.as_str())
                    .ok_or((-32602, "Missing resource URI".to_string()))?;

                match state.resources.get_resource(uri) {
                    Ok(content) => Ok(serde_json::json!({
                        "contents": [{
                            "uri": content.uri,
                            "mimeType": content.mime_type,
                            "text": content.text
                        }]
                    })),
                    Err(e) => Err((-32603, e.to_string())),
                }
            },
            "prompts/list" => {
                let prompts: Vec<Value> = state
                    .prompts
                    .list_prompts()
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "name": p.name,
                            "description": p.description,
                            "arguments": p.arguments.iter().map(|a| {
                                serde_json::json!({
                                    "name": a.name,
                                    "description": a.description,
                                    "required": a.required
                                })
                            }).collect::<Vec<Value>>()
                        })
                    })
                    .collect();
                Ok(serde_json::json!({ "prompts": prompts }))
            },
            "prompts/get" => {
                let params = params.ok_or((-32602, "Missing params".to_string()))?;
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or((-32602, "Missing prompt name".to_string()))?;
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                match state.prompts.get_prompt_messages(name, &arguments) {
                    Some(messages) => {
                        let msgs: Vec<Value> = messages
                            .iter()
                            .map(|m| {
                                serde_json::json!({
                                    "role": m.role,
                                    "content": m.content
                                })
                            })
                            .collect();
                        Ok(serde_json::json!({ "messages": msgs }))
                    },
                    None => Err((-32602, format!("Unknown prompt: {name}"))),
                }
            },
            "ping" => Ok(serde_json::json!({})),
            _ => Err((-32601, format!("Method not found: {method}"))),
        }
    }
}

#[cfg(feature = "http")]
pub use http_transport::{McpHttpState, handle_http_request};

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
    fn test_handle_initialize() {
        let mut server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let response = server.handle_request(request);

        assert!(response.contains("protocolVersion"));
        assert!(response.contains(PROTOCOL_VERSION));
        assert!(response.contains(SERVER_NAME));
    }

    #[test]
    fn test_handle_list_tools() {
        let mut server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("tools"));
        assert!(response.contains("subcog_capture"));
        assert!(response.contains("subcog_recall"));
    }

    #[test]
    fn test_handle_list_resources() {
        let mut server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"resources/list"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("resources"));
        assert!(response.contains("subcog://help"));
    }

    #[test]
    fn test_handle_list_prompts() {
        let mut server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"prompts/list"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("prompts"));
        assert!(response.contains("subcog_tutorial"));
    }

    #[test]
    fn test_handle_call_tool() {
        let mut server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"subcog_status","arguments":{}}}"#;
        let response = server.handle_request(request);

        assert!(response.contains("content"));
        assert!(response.contains("version"));
    }

    #[test]
    fn test_handle_read_resource() {
        let mut server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"subcog://help"}}"#;
        let response = server.handle_request(request);

        assert!(response.contains("contents"));
        assert!(response.contains("Subcog Help"));
    }

    #[test]
    fn test_handle_get_prompt() {
        let mut server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"prompts/get","params":{"name":"subcog_tutorial","arguments":{"familiarity":"beginner"}}}"#;
        let response = server.handle_request(request);

        assert!(response.contains("messages"));
    }

    #[test]
    fn test_handle_ping() {
        let mut server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("result"));
    }

    #[test]
    fn test_handle_unknown_method() {
        let mut server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"unknown/method"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("error"));
        assert!(response.contains("-32601"));
    }

    #[test]
    fn test_handle_parse_error() {
        let mut server = McpServer::new();
        let request = "not valid json";
        let response = server.handle_request(request);

        assert!(response.contains("error"));
        assert!(response.contains("-32700"));
    }

    #[test]
    fn test_handle_missing_params() {
        let mut server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("error"));
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
