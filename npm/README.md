# @zircote/subcog

A persistent memory system for AI coding assistants.

## Installation

```bash
# Using npm
npm install -g @zircote/subcog

# Using npx (run without installing)
npx @zircote/subcog --help

# Using pnpm
pnpm add -g @zircote/subcog

# Using yarn
yarn global add @zircote/subcog
```

## Usage

```bash
# Show help
subcog --help

# Check version
subcog --version

# Capture a memory
subcog capture --namespace learnings "Important learning about X"

# Search memories
subcog search "query terms"

# Start MCP server
subcog mcp serve
```

## Supported Platforms

| Platform | Architecture | Binary |
|----------|--------------|--------|
| macOS | Intel (x86_64) | Pre-built |
| macOS | Apple Silicon (arm64) | Pre-built |
| Linux | x86_64 (glibc) | Pre-built |
| Linux | x86_64 (musl/Alpine) | Pre-built |
| Linux | ARM64 | Pre-built |
| Windows | x64 | Cargo fallback |

## Security & Transparency

### Postinstall Script

This package includes a `postinstall` script that automatically downloads and installs the appropriate pre-built binary for your platform. This is a common pattern used by packages like esbuild, turbo, and prisma that distribute native binaries via npm.

**What the postinstall script does:**

1. Detects your platform and architecture
2. Downloads the matching pre-built binary from [GitHub Releases](https://github.com/zircote/subcog/releases)
3. Verifies the download using SHA256 checksums
4. Extracts the binary to `node_modules/@zircote/subcog/bin/`
5. Falls back to `cargo install` if download fails

**Security measures:**

- ✅ **Source code is open**: [View postinstall.js](https://github.com/zircote/subcog/blob/main/npm/scripts/postinstall.js)
- ✅ **Checksum verification**: SHA256 checksums verify binary integrity
- ✅ **Official releases only**: Downloads only from GitHub Releases
- ✅ **No arbitrary code execution**: Script only downloads and extracts binaries
- ✅ **Transparent operations**: All network requests are logged
- ✅ **Opt-out available**: Use `SUBCOG_SKIP_INSTALL=1` to skip installation

**To skip the postinstall script:**

```bash
# Skip automatic installation
SUBCOG_SKIP_INSTALL=1 npm install -g @zircote/subcog

# Then provide your own binary
export SUBCOG_BINARY_PATH=/path/to/your/subcog
npm install -g @zircote/subcog
```

**To audit the installation process:**

```bash
# Review the postinstall script source on GitHub
# https://github.com/zircote/subcog/blob/main/npm/scripts/postinstall.js

# Or view it locally after installation
cat node_modules/@zircote/subcog/scripts/postinstall.js
```

## Installation Methods

This package uses a multi-tier installation strategy:

1. **Pre-built binaries** (fastest): Downloads from GitHub Releases with checksum verification
2. **cargo install** (fallback): Builds from crates.io if binary unavailable
3. **cargo install --git** (last resort): Builds from source repository

## Environment Variables

| Variable | Description |
|----------|-------------|
| `SUBCOG_SKIP_INSTALL` | Set to `1` to skip binary installation |
| `SUBCOG_BINARY_PATH` | Path to a custom binary location |

## Alternative Installation Methods

### Homebrew (macOS)

```bash
brew tap zircote/tap
brew install subcog
```

### Cargo (Rust)

```bash
cargo install subcog
```

### From Source

```bash
cargo install --git https://github.com/zircote/subcog.git
```

## Troubleshooting

### Binary not found after installation

If you see "subcog binary not found" errors:

1. Try reinstalling:
   ```bash
   npm uninstall -g @zircote/subcog
   npm install -g @zircote/subcog
   ```

2. Check if Rust is installed for fallback compilation:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. Download the binary manually from [GitHub Releases](https://github.com/zircote/subcog/releases)

### Network issues during installation

If you're behind a proxy or firewall:

1. Set the `SUBCOG_SKIP_INSTALL=1` environment variable
2. Download the binary manually
3. Set `SUBCOG_BINARY_PATH` to the binary location

## License

MIT

## Links

- [GitHub Repository](https://github.com/zircote/subcog)
- [Documentation](https://github.com/zircote/subcog#readme)
- [Issue Tracker](https://github.com/zircote/subcog/issues)
- [Releases](https://github.com/zircote/subcog/releases)
