# Installation Guide

Subcog provides multiple installation methods to fit your workflow. Choose the one that best suits your environment.

## Quick Install

| Method | Command | Best For |
|--------|---------|----------|
| **npm/npx** | `npx @zircote/subcog --help` | Node.js projects, quick testing |
| **Homebrew** | `brew install zircote/tap/subcog` | macOS/Linux users |
| **Cargo** | `cargo install subcog` | Rust developers |
| **Docker** | `docker run ghcr.io/zircote/subcog` | Containers, CI/CD |
| **Binary** | [GitHub Releases](https://github.com/zircote/subcog/releases) | Manual installation |

## npm / npx

The npm package downloads pre-built binaries automatically for your platform.

### One-time Execution

```bash
# Run without installing
npx @zircote/subcog --help
npx @zircote/subcog status
npx @zircote/subcog capture --namespace learnings "Important insight"
```

### Global Installation

```bash
# Install globally
npm install -g @zircote/subcog

# Now use directly
subcog --help
subcog status
```

### Project-local Installation

```bash
# Add to your project
npm install --save-dev @zircote/subcog

# Run via npx or package.json scripts
npx subcog status
```

### Supported Platforms

| Platform | Architecture | Status |
|----------|--------------|--------|
| macOS | x64 (Intel) | Supported |
| macOS | arm64 (Apple Silicon) | Supported |
| Linux | x64 (glibc) | Supported |
| Linux | x64 (musl/Alpine) | Supported |
| Linux | arm64 | Supported |
| Windows | x64 | Supported |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `SUBCOG_BINARY_PATH` | Override binary location (for custom builds) |
| `SUBCOG_SKIP_INSTALL` | Set to `1` to skip binary download (CI caching) |

### Fallback Behavior

If pre-built binaries are unavailable, the postinstall script will attempt:

1. Download from GitHub Releases
2. `cargo install subcog` (if Rust is installed)
3. `cargo install --git https://github.com/zircote/subcog.git` (from source)

## Homebrew (macOS/Linux)

```bash
# Add the tap (one-time)
brew tap zircote/tap

# Install
brew install subcog

# Upgrade
brew upgrade subcog
```

## Cargo (Rust)

Requires [Rust 1.85+](https://rustup.rs/).

```bash
# From crates.io
cargo install subcog

# From GitHub (latest)
cargo install --git https://github.com/zircote/subcog.git

# Specific version
cargo install subcog --version 0.6.1
```

## Docker

Multi-architecture images are available on GitHub Container Registry.

### Quick Start

```bash
# Run subcog in a container
docker run --rm ghcr.io/zircote/subcog --help

# With persistent storage
docker run --rm \
  -v ~/.local/share/subcog:/data \
  ghcr.io/zircote/subcog status

# Capture a memory
docker run --rm \
  -v ~/.local/share/subcog:/data \
  ghcr.io/zircote/subcog capture --namespace decisions "Use Docker for deployment"
```

### Available Tags

| Tag | Description |
|-----|-------------|
| `latest` | Latest stable release |
| `0.6.1` | Specific version |
| `0.6` | Latest patch in minor version |
| `sha-abc1234` | Specific commit (for debugging) |

### Supported Architectures

- `linux/amd64` (x86_64)
- `linux/arm64` (aarch64)

### Docker Compose

```yaml
version: '3.8'
services:
  subcog:
    image: ghcr.io/zircote/subcog:latest
    volumes:
      - subcog-data:/data
    command: serve --http --port 8080
    ports:
      - "8080:8080"

volumes:
  subcog-data:
```

### MCP Server in Docker

Run subcog as an MCP server for AI agent integration:

```bash
# Stdio transport (for local clients)
docker run --rm -i \
  -v ~/.local/share/subcog:/data \
  ghcr.io/zircote/subcog serve

# HTTP transport (for network access)
docker run --rm \
  -v ~/.local/share/subcog:/data \
  -p 8080:8080 \
  ghcr.io/zircote/subcog serve --http --port 8080
```

### Image Details

| Property | Value |
|----------|-------|
| Base Image | `gcr.io/distroless/static-debian12:nonroot` |
| Size | ~15MB compressed |
| User | `nonroot` (65532) |
| Working Dir | `/data` |
| Entrypoint | `/usr/local/bin/subcog` |

## Binary Downloads

Pre-built binaries are available from [GitHub Releases](https://github.com/zircote/subcog/releases).

### Download URLs

```bash
# macOS (Intel)
curl -LO https://github.com/zircote/subcog/releases/download/v0.6.1/subcog-0.6.1-x86_64-apple-darwin.tar.gz

# macOS (Apple Silicon)
curl -LO https://github.com/zircote/subcog/releases/download/v0.6.1/subcog-0.6.1-aarch64-apple-darwin.tar.gz

# Linux (x64, glibc)
curl -LO https://github.com/zircote/subcog/releases/download/v0.6.1/subcog-0.6.1-x86_64-unknown-linux-gnu.tar.gz

# Linux (x64, musl/Alpine)
curl -LO https://github.com/zircote/subcog/releases/download/v0.6.1/subcog-0.6.1-x86_64-unknown-linux-musl.tar.gz

# Linux (arm64, musl)
curl -LO https://github.com/zircote/subcog/releases/download/v0.6.1/subcog-0.6.1-aarch64-unknown-linux-musl.tar.gz

# Windows (x64)
curl -LO https://github.com/zircote/subcog/releases/download/v0.6.1/subcog-0.6.1-x86_64-pc-windows-msvc.zip
```

### Installation Steps

```bash
# Download and extract (Unix)
curl -LO https://github.com/zircote/subcog/releases/latest/download/subcog-VERSION-TARGET.tar.gz
tar -xzf subcog-*.tar.gz
sudo mv subcog /usr/local/bin/
chmod +x /usr/local/bin/subcog

# Windows (PowerShell)
Invoke-WebRequest -Uri "https://github.com/zircote/subcog/releases/latest/download/subcog-VERSION-x86_64-pc-windows-msvc.zip" -OutFile subcog.zip
Expand-Archive subcog.zip -DestinationPath .
Move-Item subcog.exe $env:USERPROFILE\bin\
```

### Verify Checksums

All releases include SHA256 checksums in `checksums.txt`:

```bash
# Download checksums
curl -LO https://github.com/zircote/subcog/releases/download/v0.6.1/checksums.txt

# Verify (Unix)
sha256sum -c checksums.txt --ignore-missing

# Verify (macOS)
shasum -a 256 -c checksums.txt --ignore-missing
```

## Building from Source

### Prerequisites

- Rust 1.85+ (Edition 2024)
- Git 2.30+
- C compiler (for native dependencies)

### Build Steps

```bash
# Clone repository
git clone https://github.com/zircote/subcog.git
cd subcog

# Build release binary
cargo build --release

# Install to cargo bin directory
cargo install --path .

# Or copy binary manually
cp target/release/subcog /usr/local/bin/
```

### Build Options

```bash
# With all features
cargo build --release --all-features

# Minimal build (no optional features)
cargo build --release --no-default-features

# Cross-compile for musl (static binary)
cargo build --release --target x86_64-unknown-linux-musl
```

## Verification

After installation, verify subcog is working:

```bash
# Check version
subcog --version

# Check status
subcog status

# Run self-test
subcog capture --namespace testing "Installation test"
subcog recall "installation"
```

## Troubleshooting

### Binary Not Found

```bash
# Check if subcog is in PATH
which subcog

# If using npm, check node_modules
ls node_modules/.bin/subcog

# If using cargo, check cargo bin
ls ~/.cargo/bin/subcog
```

### Permission Denied

```bash
# Make binary executable (Unix)
chmod +x /path/to/subcog

# Check file ownership
ls -la /path/to/subcog
```

### Docker Permission Issues

```bash
# Run with current user
docker run --rm -u $(id -u):$(id -g) \
  -v ~/.local/share/subcog:/data \
  ghcr.io/zircote/subcog status
```

### Architecture Mismatch

```bash
# Check your architecture
uname -m

# For Apple Silicon Macs running x86 binaries via Rosetta
arch -arm64 subcog --version
```

For more troubleshooting help, see [TROUBLESHOOTING.md](TROUBLESHOOTING.md).
