# justfile — local CI parity for rust_template
# Run `just` to list all available recipes.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List available recipes
default:
    @just --list

# === Core Development ===

# Full CI check: fmt, clippy, test, doc, deny
check: fmt-check lint test doc-build deny

# Build in debug mode
build:
    cargo build

# Build in release mode
build-release:
    cargo build --release

# Run the binary
run *ARGS:
    cargo run -- {{ ARGS }}

# Run all tests
test:
    cargo test --all-features

# Run tests with stdout visible
test-verbose:
    cargo test --all-features -- --nocapture

# Run a specific test by name
test-single NAME:
    cargo test {{ NAME }}

# Build and open documentation
doc:
    cargo doc --no-deps --all-features --open

# Build documentation without opening
doc-build:
    cargo doc --no-deps --all-features

# Watch for changes and re-run tests
watch:
    cargo watch -x 'test --all-features'

# === Linting & Formatting ===

# Format code
fmt:
    cargo fmt

# Check formatting without modifying files
fmt-check:
    cargo fmt -- --check

# Run clippy with CI-equivalent flags
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run clippy and auto-fix what it can
lint-fix:
    cargo clippy --all-targets --all-features --fix --allow-dirty

# === Security & Audit ===

# Run cargo-deny supply chain checks
deny:
    cargo deny check

# Run cargo-audit advisory database check
audit:
    cargo audit --deny warnings

# Generate SBOM in SPDX format
sbom:
    cargo sbom --output-format spdx_json_2_3

# === Coverage ===

# Generate LCOV coverage report
coverage:
    cargo llvm-cov --all-features --lcov --output-path lcov.info

# Generate HTML coverage report
coverage-html:
    cargo llvm-cov --all-features --html --output-dir coverage-html

# Print coverage summary to stdout
coverage-summary:
    cargo llvm-cov --all-features --summary-only

# === Advanced Testing ===

# Check against minimum supported Rust version
msrv:
    cargo +1.92 check --all-features

# Run tests under Miri for undefined behavior detection
miri:
    cargo +nightly miri test

# Run benchmarks
bench:
    cargo bench --workspace

# Run a fuzz target for a given duration (seconds)
fuzz TARGET DURATION="60":
    cargo fuzz run {{ TARGET }} -- -max_total_time={{ DURATION }}

# Run mutation testing
mutants:
    cargo mutants --output mutants.out --json

# === Template Sync ===

# Template upstream repository
template_repo := "zircote/rust-template"
template_branch := "main"

# Sync shared tooling from the rust-template upstream
template-sync:
    #!/usr/bin/env bash
    set -euo pipefail
    TMPDIR=$(mktemp -d)
    trap 'rm -rf "$TMPDIR"' EXIT
    echo "Fetching latest template from {{ template_repo }}..."
    git clone --depth 1 --branch {{ template_branch }} \
        "https://github.com/{{ template_repo }}.git" "$TMPDIR/template" 2>/dev/null
    SYNC_PATHS=( \
        ".claude/commands/spec-orchestrator.md" \
        "clippy.toml" \
        "rustfmt.toml" \
        "deny.toml" \
    )
    for p in "${SYNC_PATHS[@]}"; do
        src="$TMPDIR/template/$p"
        if [ -e "$src" ]; then
            mkdir -p "$(dirname "$p")"
            if [ -d "$src" ]; then
                cp -R "$src/." "$p/"
            else
                cp "$src" "$p"
            fi
            echo "  synced: $p"
        else
            echo "  skip (not in template): $p"
        fi
    done
    echo "Done. Review changes with: git diff"

# === Release ===

# Dry-run a crates.io publish
publish-dry:
    cargo publish --dry-run

# Generate changelog for the latest release
changelog:
    git-cliff --latest --strip header
