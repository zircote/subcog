//! Template rendering engine.
//!
//! Provides rendering capabilities for context templates with variable substitution,
//! iteration support, and output format conversion.

mod template_renderer;

pub use template_renderer::{RenderContext, RenderValue, TemplateRenderer};
