# Security Awareness (COMP-H10)

## Purpose
Security training and awareness for subcog users.

## Security Best Practices

### For Developers

#### Configuration
- Never commit secrets to version control
- Use environment variables for sensitive config
- Enable security features in production

#### Code Review
- Check for SQL injection vulnerabilities
- Verify path traversal protection
- Review authentication/authorization

#### Dependencies
- Run `cargo deny check` before releases
- Monitor security advisories
- Update dependencies regularly

### For Operators

#### Deployment
- Use HTTPS for HTTP transport
- Configure JWT authentication
- Enable TLS for PostgreSQL

#### Monitoring
- Enable audit logging
- Review logs for anomalies
- Set up alerts for failures

#### Backup
- Regular backups of SQLite database
- Test recovery procedures
- Document restore process

### For End Users

#### Safe Usage
- Don't capture secrets/credentials
- Review captured content before sharing
- Report suspicious behavior

## Security Contacts
- Security issues: [security@example.com]
- General questions: [support@example.com]

## Training Resources
- Subcog security documentation
- OWASP guidelines
- SOC 2 compliance training
