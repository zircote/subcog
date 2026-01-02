# Backup and Recovery (COMP-H3)

## Purpose
Define backup and disaster recovery procedures.

## Backup Strategy

### Data Classification
| Data Type | Backup Frequency | Retention | Location |
|-----------|------------------|-----------|----------|
| Git Notes | Per-commit | Indefinite | Git remote |
| SQLite Index | [Daily] | [30 days] | [Define] |
| PostgreSQL | [Daily] | [30 days] | [Define] |
| Audit Logs | [Daily] | [1 year] | [Define] |

### Backup Procedures
1. Automated backups via [cron/systemd/scheduler]
2. Encryption in transit and at rest
3. Off-site replication

## Recovery Procedures

### Recovery Time Objectives
- Git Notes: < 1 hour (git fetch)
- SQLite Index: < 4 hours (rebuild from git notes)
- Full System: < 24 hours

### Recovery Steps
1. Verify backup integrity
2. Restore data to new environment
3. Validate data consistency
4. Update DNS/routing if needed
5. Verify service health

## Technical Implementation
- Git notes: `git fetch origin refs/notes/*:refs/notes/*`
- Index rebuild: `subcog reindex`
- SQLite backup: `sqlite3 index.db ".backup backup.db"`

## Testing
- Frequency: [Quarterly]
- Documentation: [Location]
