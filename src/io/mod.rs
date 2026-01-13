//! Import/Export I/O subsystem.
//!
//! Provides bulk memory import and structured export capabilities with support
//! for multiple file formats (JSON, YAML, CSV, Parquet).
//!
//! # Architecture
//!
//! The I/O subsystem uses a clean trait-based architecture:
//!
//! - **Format adapters** implement [`ImportSource`] and [`ExportSink`] traits
//! - **Validation layer** normalizes and validates imported data
//! - **Services** orchestrate format parsing, validation, and storage
//!
//! # Supported Formats
//!
//! | Format | Import | Export | Notes |
//! |--------|--------|--------|-------|
//! | JSON | ✓ | ✓ | Newline-delimited (NDJSON) or array |
//! | YAML | ✓ | ✓ | Document stream |
//! | CSV | ✓ | ✓ | Configurable column mapping |
//! | Parquet | - | ✓ | Requires `parquet-export` feature |
//!
//! # Examples
//!
//! ## Import memories from JSON
//!
//! ```rust,ignore
//! use subcog::io::{ImportService, ImportOptions, Format};
//! use std::fs::File;
//!
//! let file = File::open("memories.json")?;
//! let result = service.import_from_reader(file, ImportOptions {
//!     format: Format::Json,
//!     skip_duplicates: true,
//!     ..Default::default()
//! })?;
//! println!("Imported {} memories", result.imported);
//! ```
//!
//! ## Export memories to CSV
//!
//! ```rust,ignore
//! use subcog::io::{ExportService, ExportOptions, Format};
//! use std::fs::File;
//!
//! let file = File::create("memories.csv")?;
//! let result = service.export_to_writer(file, ExportOptions {
//!     format: Format::Csv,
//!     filter: Some("ns:decisions since:7d".to_string()),
//!     ..Default::default()
//! })?;
//! println!("Exported {} memories", result.exported);
//! ```

pub mod formats;
pub mod services;
pub mod traits;
pub mod validation;

// Re-exports for convenience
pub use formats::Format;
pub use services::export::{ExportOptions, ExportResult, ExportService};
pub use services::import::{ImportOptions, ImportProgress, ImportResult, ImportService};
pub use traits::{ExportSink, ImportSource, ImportedMemory};
pub use validation::{ImportValidator, ValidationIssue, ValidationResult};
