# Backup and Recovery (COMP-H3)

## Purpose
Define backup and disaster recovery procedures.

## Backup Strategy

### Data Classification
| Data Type | Backup Frequency | Retention | Location |
|-----------|------------------|-----------|----------|
| SQLite Database | Daily | 30 days | Local + remote |
| PostgreSQL | Daily | 30 days | Database backup |
| Vector Index | On-demand | Rebuild from DB | Local |
| Audit Logs | Daily | 1 year | Secure storage |

### Backup Procedures
1. Automated backups via cron/systemd/scheduler
2. Encryption in transit and at rest
3. Off-site replication

## Recovery Procedures

### Recovery Time Objectives
- SQLite Database: < 1 hour (restore from backup)
- Vector Index: < 2 hours (rebuild from database)
- Full System: < 4 hours

### Recovery Steps
1. Verify backup integrity
2. Restore data to new environment
3. Rebuild vector index with `subcog reindex`
4. Validate data consistency
5. Update DNS/routing if needed
6. Verify service health

## Technical Implementation

### SQLite Backup
```bash
# Create backup
sqlite3 ~/.local/share/subcog/subcog.db ".backup backup.db"

# Restore from backup
cp backup.db ~/.local/share/subcog/subcog.db
```

### PostgreSQL Backup
```bash
# Create backup
pg_dump -Fc subcog > subcog_backup.dump

# Restore from backup
pg_restore -d subcog subcog_backup.dump
```

### Index Rebuild
```bash
# Rebuild search index and vectors from persistence layer
subcog reindex
```

### Remote Sync
```bash
# Push memories to remote for backup
subcog sync push

# Fetch memories from remote
subcog sync fetch
```

## Testing
- Frequency: Quarterly
- Documentation: Recovery test results in compliance/recovery-tests/
