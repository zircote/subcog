//! Structured logging.

use std::fmt;
use std::path::PathBuf;

use serde_json::{Map, Number, Value};
use tracing::field::{Field, Visit};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::field::RecordFields;
use tracing_subscriber::fmt::format::{FormatFields, Writer};

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
    /// Optional log file path (logs to stderr if None).
    pub file: Option<PathBuf>,
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

        let file = settings
            .and_then(|config| config.file.as_ref())
            .map(PathBuf::from);

        Self {
            format: log_format_from_env_override(format),
            filter: filter_from_env_override(filter),
            file: log_file_from_env_override(file),
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

/// Redactor for sensitive log fields.
#[derive(Debug, Clone)]
pub struct LogRedactor {
    sensitive_fields: Vec<&'static str>,
    max_len: usize,
}

impl LogRedactor {
    /// Creates a redactor with default rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sensitive_fields: vec![
                "content",
                "prompt",
                "token",
                "secret",
                "password",
                "authorization",
                "api_key",
                "api-key",
                "jwt",
            ],
            max_len: 120,
        }
    }

    /// Redacts a value based on field name.
    #[must_use]
    pub fn redact_field(&self, field: &str, value: &str) -> String {
        let field_lower = field.to_lowercase();
        if self
            .sensitive_fields
            .iter()
            .any(|needle| field_lower.contains(needle))
        {
            return "[REDACTED]".to_string();
        }

        if value.chars().count() > self.max_len {
            let truncated: String = value.chars().take(self.max_len).collect();
            return format!("{truncated}...(truncated)");
        }

        value.to_string()
    }
}

impl Default for LogRedactor {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON field formatter with redaction support.
#[derive(Debug, Clone, Default)]
pub struct RedactingJsonFields {
    redactor: LogRedactor,
}

impl RedactingJsonFields {
    /// Creates a redacting JSON field formatter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            redactor: LogRedactor::new(),
        }
    }
}

impl Default for RedactingJsonFields {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatFields for RedactingJsonFields {
    fn format_fields<R: RecordFields>(&self, writer: Writer<'_>, fields: R) -> fmt::Result {
        let mut visitor = RedactingVisitor::new(&self.redactor);
        fields.record(&mut visitor);
        let json = serde_json::to_string(&visitor.values).map_err(|_| fmt::Error)?;
        writer.write_str(&json)
    }
}

struct RedactingVisitor<'a> {
    values: Map<String, Value>,
    redactor: &'a LogRedactor,
}

impl<'a> RedactingVisitor<'a> {
    fn new(redactor: &'a LogRedactor) -> Self {
        Self {
            values: Map::new(),
            redactor,
        }
    }

    fn insert_str(&mut self, field: &Field, value: &str) {
        let redacted = self.redactor.redact_field(field.name(), value);
        self.values
            .insert(field.name().to_string(), Value::String(redacted));
    }

    fn insert_number(&mut self, field: &Field, number: Number) {
        self.values
            .insert(field.name().to_string(), Value::Number(number));
    }
}

impl Visit for RedactingVisitor<'_> {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert_number(field, Number::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert_number(field, Number::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.values
            .insert(field.name().to_string(), Value::Bool(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        let number = Number::from_f64(value).unwrap_or_else(|| Number::from(0_u64));
        self.insert_number(field, number);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.insert_str(field, value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let formatted = format!("{value:?}");
        self.insert_str(field, &formatted);
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.insert_str(field, &value.to_string());
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
    let normalized = level.trim().to_lowercase();
    if normalized.contains('=') || normalized.contains(',') {
        normalized
    } else {
        format!("subcog={normalized}")
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

fn log_file_from_env_override(default: Option<PathBuf>) -> Option<PathBuf> {
    std::env::var("SUBCOG_LOG_FILE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or(default)
}
