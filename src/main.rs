//! Binary entry point for subcog.
//!
//! This binary provides the CLI interface for the subcog memory system.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
// Allow print_stderr in main binary for CLI output
#![allow(clippy::print_stderr)]

use subcog::{Config, add, divide};

/// Main entry point.
///
/// Currently a placeholder demonstrating basic library usage.
/// Will be replaced with clap-based CLI in Phase 1.
fn main() {
    // Example usage
    let config = Config::new().with_verbose(true);

    if config.verbose {
        eprintln!("Running subcog with verbose mode enabled");
    }

    // Demonstrate add function
    let sum = add(2, 3);
    eprintln!("2 + 3 = {sum}");

    // Demonstrate divide function with error handling
    match divide(10, 2) {
        Ok(result) => eprintln!("10 / 2 = {result}"),
        Err(e) => eprintln!("Error: {e}"),
    }

    // Demonstrate error case
    match divide(10, 0) {
        Ok(result) => eprintln!("10 / 0 = {result}"),
        Err(e) => eprintln!("Expected error: {e}"),
    }
}
