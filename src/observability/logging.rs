//! Structured logging.

use tracing_subscriber::EnvFilter;

use crate::config::LoggingSettings;

/// Logging output format.
#[derive(Debug, Clone, Copy)]
pub enum LogFormat {
    /// JSON structured logs.
    Json,
    /// Human-friendly logs for local debugging.
    Pretty,
}

/// Logging configuration.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Log format.
    pub format: LogFormat,
    /// Log filter (e.g., `subcog=info`).
    pub filter: EnvFilter,
}

impl LoggingConfig {
    /// Builds logging configuration from environment variables.
    #[must_use]
    pub fn from_env(verbose: bool) -> Self {
        Self::from_settings(None, verbose)
    }

    /// Builds logging configuration from config settings with env overrides.
    #[must_use]
    pub fn from_settings(settings: Option<&LoggingSettings>, verbose: bool) -> Self {
        let format = settings
            .and_then(|config| config.format.as_deref())
            .and_then(parse_log_format)
            .unwrap_or(LogFormat::Json);

        let filter = settings
            .and_then(|config| config.filter.as_ref())
            .map(|filter| EnvFilter::new(filter.clone()))
            .or_else(|| {
                settings
                    .and_then(|config| config.level.as_ref())
                    .map(|level| EnvFilter::new(normalize_level(level.clone())))
            })
            .unwrap_or_else(|| default_filter(verbose));

        Self {
            format: log_format_from_env_override(format),
            filter: filter_from_env_override(filter),
        }
    }
}

/// Logger for structured logging.
pub struct Logger;

impl Logger {
    /// Creates a new logger.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_log_format(value: &str) -> Option<LogFormat> {
    match value.to_lowercase().as_str() {
        "pretty" => Some(LogFormat::Pretty),
        "json" => Some(LogFormat::Json),
        _ => None,
    }
}

fn log_format_from_env_override(default: LogFormat) -> LogFormat {
    std::env::var("SUBCOG_LOG_FORMAT")
        .map_or(default, |value| parse_log_format(&value).unwrap_or(default))
}

fn filter_from_env_override(default_filter: EnvFilter) -> EnvFilter {
    if let Ok(filter) = std::env::var("SUBCOG_LOG_FILTER") {
        return EnvFilter::new(filter);
    }

    if let Ok(level) = std::env::var("SUBCOG_LOG_LEVEL") {
        return EnvFilter::new(normalize_level(level));
    }

    if let Ok(filter) = EnvFilter::try_from_default_env() {
        return filter;
    }

    default_filter
}

fn normalize_level(level: String) -> String {
    if level.contains('=') || level.contains(',') {
        level
    } else {
        format!("subcog={level}")
    }
}

fn default_filter(verbose: bool) -> EnvFilter {
    let default_level = if verbose {
        "subcog=debug"
    } else {
        "subcog=info"
    };
    EnvFilter::new(default_level)
}
