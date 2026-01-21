# Security Policy

## Postinstall Script Security

This npm package includes a `postinstall` script that downloads and installs pre-built binaries. This document explains the security measures and provides transparency about the installation process.

### What the Postinstall Script Does

The postinstall script (`scripts/postinstall.js`) performs the following operations:

1. **Platform Detection**: Detects your operating system and CPU architecture
2. **Binary Download**: Downloads the matching pre-built binary from GitHub Releases
3. **Checksum Verification**: Verifies the download using SHA256 checksums from `checksums.txt`
4. **Extraction**: Extracts the binary to `bin/` directory
5. **Fallback**: Falls back to `cargo install` if download fails

### Security Measures

#### 1. Source Transparency

- **Open Source**: Full source code available at [github.com/zircote/subcog](https://github.com/zircote/subcog)
- **Auditable**: The postinstall script is <500 lines and easy to audit
- **No obfuscation**: All code is readable JavaScript with clear comments

#### 2. Cryptographic Verification

- **SHA256 checksums**: Every binary download is verified against published checksums
- **Checksum file**: `checksums.txt` is downloaded from the same release
- **Automatic verification**: Installation fails if checksums don't match

#### 3. Limited Network Access

- **Official sources only**: Downloads only from `github.com/zircote/subcog/releases`
- **No third-party domains**: No external APIs or tracking services
- **Redirect following**: Handles GitHub's CDN redirects transparently

#### 4. No Arbitrary Code Execution

- **Fixed commands only**: Uses `spawn()` with argument arrays (no shell injection)
- **No eval**: Never uses `eval()`, `Function()`, or dynamic code generation
- **Safe system calls**: Only calls `cargo` and `ldd` with fixed arguments

#### 5. User Control

- **Opt-out**: Set `SUBCOG_SKIP_INSTALL=1` to skip installation
- **Custom binary**: Use `SUBCOG_BINARY_PATH` to provide your own binary
- **Transparent logging**: All operations are logged to console

### Installation Options

If you prefer not to run the postinstall script, you have several alternatives:

#### Option 1: Skip Postinstall

```bash
# Install without running postinstall
SUBCOG_SKIP_INSTALL=1 npm install -g @zircote/subcog

# Manually download binary from GitHub Releases
curl -LO https://github.com/zircote/subcog/releases/latest/download/subcog-VERSION-TARGET.tar.gz
tar xzf subcog-VERSION-TARGET.tar.gz
mv subcog ~/.npm-global/lib/node_modules/@zircote/subcog/bin/
```

#### Option 2: Use Custom Binary

```bash
# Build from source
cargo install subcog

# Point npm package to your binary
export SUBCOG_BINARY_PATH=$(which subcog)
npm install -g @zircote/subcog
```

#### Option 3: Use Alternative Distribution

```bash
# Homebrew (macOS/Linux)
brew install zircote/tap/subcog

# Cargo (Rust package manager)
cargo install subcog

# Docker
docker run ghcr.io/zircote/subcog
```

### Verifying Package Integrity

#### Verify npm Package

```bash
# Check package signature (requires npm@9+)
npm audit signatures

# View package contents before installing
npm pack @zircote/subcog --dry-run
```

#### Verify Binary Checksums

```bash
# Download checksums file
curl -LO https://github.com/zircote/subcog/releases/download/v0.13.1/checksums.txt

# Verify your downloaded binary
sha256sum subcog-0.13.1-x86_64-unknown-linux-gnu.tar.gz
# Compare with checksums.txt
```

### Reporting Security Issues

**Do NOT open a public GitHub issue for security vulnerabilities.**

Please report security issues via:

1. [GitHub Security Advisories](https://github.com/zircote/subcog/security/advisories/new) (preferred)
2. Email to the maintainers (see GitHub profile)

We will:
- Acknowledge receipt within 48 hours
- Provide status updates within 7 days
- Resolve critical issues within 30 days

### Security Track Record

This package follows security best practices:

- No unsafe Rust code (`#![forbid(unsafe_code)]`)
- Regular dependency audits with `cargo-audit` and `cargo-deny`
- Vulnerability scanning in CI/CD pipeline
- Supply chain security with npm provenance

### Comparison to Similar Packages

This installation approach is used by many popular packages:

| Package | Postinstall Script | Purpose |
|---------|-------------------|---------|
| esbuild | ✅ Yes | Download platform-specific binary |
| turbo | ✅ Yes | Download platform-specific binary |
| prisma | ✅ Yes | Download database engines |
| @swc/core | ✅ Yes | Download platform-specific binary |
| subcog | ✅ Yes | Download platform-specific binary |

### References

- [npm postinstall documentation](https://docs.npmjs.com/cli/v9/using-npm/scripts#life-cycle-scripts)
- [GitHub Releases](https://github.com/zircote/subcog/releases)
- [Rust security policy](https://github.com/zircote/subcog/blob/main/SECURITY.md)

## License

MIT - See [LICENSE](LICENSE) file in repository
