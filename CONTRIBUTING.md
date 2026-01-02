# Contributing to Subcog

Thank you for your interest in contributing to Subcog! This guide will help you get your development environment set up and walk you through the contribution process.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Development Setup](#development-setup)
3. [Project Structure](#project-structure)
4. [Build Commands](#build-commands)
5. [Code Style](#code-style)
6. [Testing](#testing)
7. [Making Changes](#making-changes)
8. [Pull Request Process](#pull-request-process)
9. [Troubleshooting](#troubleshooting)

## Prerequisites

### Required Tools

| Tool | Version | Installation |
|------|---------|--------------|
| **Rust** | 1.85+ | [rustup.rs](https://rustup.rs) |
| **Git** | 2.30+ | [git-scm.com](https://git-scm.com) |
| **cargo-deny** | latest | `cargo install cargo-deny` |

### Optional Tools

| Tool | Purpose | Installation |
|------|---------|--------------|
| **rust-analyzer** | IDE support | [rust-analyzer.github.io](https://rust-analyzer.github.io) |
| **MIRI** | Undefined behavior detection | `rustup +nightly component add miri` |
| **cargo-watch** | Auto-rebuild on changes | `cargo install cargo-watch` |
| **cargo-expand** | Macro expansion | `cargo install cargo-expand` |

### Verify Installation

```bash
# Check Rust version (must be 1.85+)
rustc --version

# Check cargo-deny
cargo deny --version

# Check git version
git --version
```

## Development Setup

### 1. Clone the Repository

```bash
git clone https://github.com/zircote/subcog.git
cd subcog
```

### 2. Install Dependencies

```bash
# This will fetch all dependencies and compile in debug mode
cargo build
```

### 3. Run the Test Suite

```bash
# Run all tests
cargo test

# Run with output visible
cargo test -- --nocapture
```

### 4. Verify All Checks Pass

```bash
# Run the full CI check locally
make ci

# Or run each step manually:
cargo fmt -- --check          # Format check
cargo clippy --all-targets --all-features -- -D warnings  # Linting
cargo test                    # Tests
cargo doc --no-deps           # Documentation
cargo deny check              # Supply chain security
```

### 5. IDE Setup (Recommended)

For VS Code with rust-analyzer:

```json
// .vscode/settings.json
{
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.check.extraArgs": ["--all-targets", "--all-features"],
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
```

## Project Structure

```
src/
├── lib.rs                    # Library entry point, Error type, re-exports
├── main.rs                   # CLI entry point, argument parsing
│
├── models/                   # Data structures
│   ├── memory.rs            # Memory, MemoryId
│   ├── domain.rs            # Domain, Namespace (11 variants)
│   ├── prompt.rs            # PromptTemplate, validation
│   └── ...
│
├── storage/                  # Three-layer storage abstraction
│   ├── traits/              # PersistenceBackend, IndexBackend, VectorBackend
│   ├── persistence/         # Git Notes, PostgreSQL, Filesystem
│   ├── index/               # SQLite+FTS5, PostgreSQL, Redis
│   └── vector/              # usearch, pgvector, Redis
│
├── services/                 # Business logic
│   ├── capture.rs           # CaptureService
│   ├── recall.rs            # RecallService (RRF fusion search)
│   ├── deduplication/       # Duplicate detection (3-tier)
│   └── ...
│
├── hooks/                    # Claude Code hooks (5 handlers)
│   ├── session_start.rs     # Context injection
│   ├── user_prompt.rs       # Signal detection
│   ├── search_intent.rs     # Intent classification
│   └── ...
│
├── mcp/                      # MCP server
│   ├── tools/               # Tool implementations
│   ├── resources.rs         # URN resource handlers
│   └── prompts.rs           # Built-in prompts
│
├── security/                 # Secret/PII detection, redaction
├── embedding/                # FastEmbed integration
├── llm/                      # LLM providers (Anthropic, OpenAI, Ollama)
└── observability/            # Tracing, metrics, logging

tests/                        # Integration tests
benches/                      # Performance benchmarks
docs/                         # Documentation
```

## Build Commands

### Essential Commands

```bash
# Build (debug)
cargo build

# Build (release)
cargo build --release

# Run tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run benchmarks
cargo bench
```

### Quality Checks

```bash
# Format code
cargo fmt

# Check formatting (CI mode)
cargo fmt -- --check

# Lint with clippy
cargo clippy --all-targets --all-features

# Lint and fail on warnings (CI mode)
cargo clippy --all-targets --all-features -- -D warnings

# Generate documentation
cargo doc --open

# Supply chain security check
cargo deny check
```

### Advanced Commands

```bash
# Run with MIRI (undefined behavior detection)
cargo +nightly miri test

# Expand macros (for debugging)
cargo expand --lib models::prompt

# Watch mode (auto-rebuild)
cargo watch -x build -x test
```

## Code Style

### Linting Rules

We use `clippy::pedantic` and `clippy::nursery` lints. Key rules:

| Rule | Requirement |
|------|-------------|
| **No panics** | Use `Result` types; never `unwrap()` or `expect()` in library code |
| **No unsafe** | `#![forbid(unsafe_code)]` - exceptions require justification |
| **Documentation** | All public items must have doc comments with examples |
| **Line length** | 100 characters maximum |

### Error Handling

```rust
// ✅ Good - Returns Result
pub fn parse(input: &str) -> Result<Value, ParseError> {
    if input.is_empty() {
        return Err(ParseError::EmptyInput);
    }
    Ok(value)
}

// ❌ Bad - Panics
pub fn parse(input: &str) -> Value {
    input.parse().unwrap()  // Never do this
}
```

### Documentation

All public items must have documentation:

```rust
/// Captures a memory to persistent storage.
///
/// # Arguments
///
/// * `request` - The capture request containing content and metadata.
///
/// # Returns
///
/// A [`CaptureResult`] with the memory ID and URN.
///
/// # Errors
///
/// Returns [`Error::ContentBlocked`] if secrets are detected.
///
/// # Examples
///
/// ```rust
/// use subcog::{CaptureService, CaptureRequest};
///
/// let service = CaptureService::new(config)?;
/// let result = service.capture(request)?;
/// # Ok::<(), subcog::Error>(())
/// ```
pub fn capture(&self, request: CaptureRequest) -> Result<CaptureResult> {
    // ...
}
```

### Builder Pattern

Use builder pattern for complex configuration:

```rust
let config = Config::new()
    .with_timeout(Duration::from_secs(30))
    .with_retries(3);
```

## Testing

### Test Organization

| Location | Purpose |
|----------|---------|
| `src/*.rs` with `#[cfg(test)]` | Unit tests |
| `tests/` | Integration tests |
| Doc comments with `///` | Doc tests |
| `benches/` | Performance benchmarks |

### Running Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_capture_memory

# Tests in specific module
cargo test services::capture

# Integration tests only
cargo test --test integration_test
```

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_case() {
        let result = function_under_test(valid_input);
        assert_eq!(result, expected_output);
    }

    #[test]
    fn test_error_case() {
        let result = function_under_test(invalid_input);
        assert!(matches!(result, Err(Error::InvalidInput(_))));
    }
}
```

## Making Changes

### 1. Create a Branch

```bash
git checkout -b feat/my-feature
# or
git checkout -b fix/issue-123
```

### 2. Make Changes

- Write code following the style guide
- Add tests for new functionality
- Update documentation as needed

### 3. Run Checks Locally

```bash
# Must pass before committing
make ci
```

### 4. Commit Changes

Use conventional commit format:

```
feat(hooks): add search intent caching
fix(recall): handle empty query gracefully
docs: update API documentation
test(security): add bypass attack tests
refactor(services): extract deduplication module
```

## Pull Request Process

### 1. Push Your Branch

```bash
git push -u origin feat/my-feature
```

### 2. Create Pull Request

- Use a clear title following conventional commits
- Reference any related issues
- Describe what changed and why

### 3. PR Requirements

All PRs must:

- [ ] Pass CI (format, clippy, tests, doc, deny)
- [ ] Include tests for new functionality
- [ ] Update documentation if needed
- [ ] Have no merge conflicts

### 4. Review Process

- At least one approval required
- Address feedback promptly
- Keep PR scope focused

## Troubleshooting

### Build Failures

**"error: could not compile `subcog`"**

```bash
# Clean and rebuild
cargo clean
cargo build
```

**Clippy warnings**

```bash
# See all warnings
cargo clippy --all-targets --all-features 2>&1 | less

# Fix auto-fixable issues
cargo clippy --fix --all-targets --all-features
```

### Test Failures

**Tests timing out**

```bash
# Run with extended timeout
cargo test -- --test-threads=1
```

**Flaky tests**

```bash
# Run test multiple times
for i in {1..10}; do cargo test test_name || break; done
```

### IDE Issues

**rust-analyzer not working**

```bash
# Regenerate cargo metadata
cargo clean
cargo check
```

### Supply Chain Issues

**cargo-deny check fails**

```bash
# See what's wrong
cargo deny check 2>&1 | less

# Update deny.toml if needed (e.g., new license)
```

## Getting Help

- **Issues**: [github.com/zircote/subcog/issues](https://github.com/zircote/subcog/issues)
- **Discussions**: [github.com/zircote/subcog/discussions](https://github.com/zircote/subcog/discussions)
- **Documentation**: [docs/](docs/)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
