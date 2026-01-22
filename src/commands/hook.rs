//! Hook command handler.
//!
//! Contains the implementation of the `hook` CLI command for
//! Claude Code hook event handling.

use std::path::Path;
use subcog::config::SubcogConfig;
use subcog::context::GitContext;
use subcog::hooks::{
    AdaptiveContextConfig, HookHandler, PostToolUseHandler, PreCompactHandler, SessionStartHandler,
    StopHandler, UserPromptHandler,
};
use subcog::models::{EventMeta, MemoryEvent};
use subcog::observability::{
    RequestContext, current_request_id, enter_request_context, flush_metrics,
};
use subcog::security::record_event;
use subcog::services::ContextBuilderService;
use subcog::storage::index::SqliteBackend;
use subcog::{CaptureService, RecallService, SyncService};
use tracing::info_span;

use super::HookEvent;

/// Hook command.
pub fn cmd_hook(event: HookEvent, config: &SubcogConfig) -> Result<(), Box<dyn std::error::Error>> {
    use subcog::cli::build_hook_llm_provider;

    // Set instance label for metrics including hook type to prevent metric collision
    // Each hook type gets its own instance (hooks-session-start, hooks-user-prompt-submit, etc.)
    let instance_label = format!("hooks-{}", event.as_str());
    subcog::observability::set_instance_label(&instance_label);

    let request_context = RequestContext::new();
    let request_id = request_context.request_id().to_string();
    let _request_guard = enter_request_context(request_context);
    let span = info_span!(
        "subcog.hook.invoke",
        request_id = %request_id,
        component = "hooks",
        operation = "invoke",
        hook = event.as_str()
    );
    let _span_guard = span.enter();

    // Read input from stdin as a string
    let input = read_hook_input()?;

    // Try to initialize services for hooks (may fail if no data dir)
    // Use config.data_dir to respect user's config.toml setting
    let recall_service = try_init_recall_service(&config.data_dir);

    // Get repo path for project facet metadata
    let cwd = std::env::current_dir().ok();
    let mut capture_config = subcog::config::Config::from(config.clone());
    if let Some(path) = cwd.as_ref() {
        capture_config = capture_config.with_repo_path(path);
    }
    let capture_service = CaptureService::new(capture_config.clone());
    let sync_service = SyncService::default();

    let hook_name = event.as_str();
    record_event(MemoryEvent::HookInvoked {
        meta: EventMeta::new("hooks", current_request_id()),
        hook: hook_name.to_string(),
    });

    let response = match event {
        HookEvent::SessionStart => {
            // SessionStart with context builder for memory injection
            let handler = if let Some(recall) = recall_service {
                SessionStartHandler::new()
                    .with_context_builder(ContextBuilderService::with_recall(recall))
            } else {
                SessionStartHandler::new()
            };
            handler.handle(&input)
        },
        HookEvent::UserPromptSubmit => {
            let context_config =
                AdaptiveContextConfig::from_search_intent_config(&config.search_intent);
            // Create separate capture service for auto-capture in this handler
            let handler_capture_service = CaptureService::new(capture_config);
            let mut handler = UserPromptHandler::new()
                .with_search_intent_config(config.search_intent.clone())
                .with_context_config(context_config)
                .with_capture_service(handler_capture_service)
                .with_auto_capture(config.features.auto_capture);
            if let Some(provider) = build_hook_llm_provider(config) {
                handler = handler.with_llm_provider(provider);
            }
            handler.handle(&input)
        },
        HookEvent::PostToolUse => {
            // PostToolUse with recall service for memory surfacing
            let handler = if let Some(recall) = recall_service {
                PostToolUseHandler::new().with_recall(recall)
            } else {
                PostToolUseHandler::new()
            };
            handler.handle(&input)
        },
        HookEvent::PreCompact => {
            // PreCompact with capture service for auto-capture
            let handler = PreCompactHandler::new().with_capture(capture_service);
            handler.handle(&input)
        },
        HookEvent::Stop => {
            // Stop with sync service for session-end sync
            let handler = StopHandler::new().with_sync(sync_service);
            handler.handle(&input)
        },
    };

    let response = match response {
        Ok(response) => response,
        Err(err) => {
            record_event(MemoryEvent::HookFailed {
                meta: EventMeta::new("hooks", current_request_id()),
                hook: hook_name.to_string(),
                error: err.to_string(),
            });
            return Err(Box::new(err));
        },
    };

    // Output response (already JSON string)
    println!("{response}");

    // Delay to ensure spawned threads complete metric recording.
    // Note: LLM threads use recv_timeout, so if HTTP timeout < search_intent timeout,
    // the thread will complete before recv_timeout expires. This delay is just a buffer
    // for any remaining metric recording after channel communication.
    std::thread::sleep(std::time::Duration::from_millis(250));

    // Flush metrics to push gateway before exit
    flush_metrics();

    Ok(())
}

/// Tries to initialize a recall service with `SQLite` backend.
///
/// Uses the provided `data_dir` from config to ensure hooks use the same
/// data directory as the MCP server (respects config.toml `data_dir` setting).
fn try_init_recall_service(data_dir: &Path) -> Option<RecallService> {
    if std::fs::create_dir_all(data_dir).is_err() {
        return None;
    }

    let db_path = data_dir.join("index.db");
    let recall = SqliteBackend::new(&db_path)
        .ok()
        .map(RecallService::with_index)?;

    let scope_filter = GitContext::from_cwd()
        .project_id
        .map(|project_id| subcog::SearchFilter::new().with_project_id(project_id));

    match scope_filter {
        Some(filter) => Some(recall.with_scope_filter(filter)),
        None => Some(recall),
    }
}

/// Reads hook input from stdin as a string.
fn read_hook_input() -> Result<String, Box<dyn std::error::Error>> {
    use std::io::{self, Read};

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    if input.trim().is_empty() {
        Ok("{}".to_string())
    } else {
        Ok(input)
    }
}
