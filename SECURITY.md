# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.9.x   | :white_check_mark: |
| 0.8.x   | :white_check_mark: |
| < 0.8   | :x:                |

## Reporting a Vulnerability

We take the security of Subcog seriously. If you discover a security vulnerability, please report it responsibly.

### How to Report

**Do NOT open a public GitHub issue for security vulnerabilities.**

Instead, please use one of the following methods:

1. **GitHub Security Advisories** (Preferred): Report via [GitHub Security Advisories](https://github.com/zircote/subcog/security/advisories/new)

2. **Email**: Send details to the repository maintainers (see GitHub profile)

### What to Include

Please provide as much information as possible:

- Description of the vulnerability
- Steps to reproduce
- Affected versions
- Potential impact
- Any suggested fixes (optional)

### Response Timeline

- **Initial Response**: Within 48 hours
- **Status Update**: Within 7 days
- **Resolution Target**: Within 30 days for critical issues

### What to Expect

1. **Acknowledgment**: We'll confirm receipt of your report
2. **Assessment**: We'll evaluate the severity and impact
3. **Updates**: We'll keep you informed of our progress
4. **Credit**: With your permission, we'll credit you in the release notes

## Security Measures

Subcog implements several security measures:

- **No unsafe code**: `#![forbid(unsafe_code)]` enforced
- **Dependency auditing**: Regular `cargo-audit` and `cargo-deny` checks
- **Secret detection**: Built-in PII/secret detection prevents accidental capture
- **Vulnerability scanning**: Trivy scanning in CI/CD pipeline
- **Pinned dependencies**: All CI actions use SHA-pinned versions

## Scope

The following are in scope for security reports:

- Memory injection vulnerabilities
- Secret/PII leakage
- Authentication/authorization bypass (for MCP server)
- Dependency vulnerabilities
- Cryptographic weaknesses

The following are out of scope:

- Denial of service via resource exhaustion (single-user tool)
- Issues requiring physical access
- Social engineering attacks
