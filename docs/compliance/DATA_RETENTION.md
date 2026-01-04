# Data Retention Policy (COMP-H7)

## Purpose
Define data retention and deletion policies.

## Retention Schedule

| Data Type | Retention | Basis | Deletion Method |
|-----------|-----------|-------|-----------------|
| Memories | Indefinite | User value | Manual via API |
| Audit Logs | 1 year | Compliance | Auto-rotate |
| Session Data | 24 hours | Operational | Auto-expire |
| Temp Files | 1 hour | Processing | Auto-cleanup |

## GDPR Rights

### Right to Erasure (Article 17)
Users can request deletion of their memories:
```bash
subcog delete --id <memory_id>
```

Technical implementation:
- `PersistenceBackend::delete()` - removes from SQLite database
- `IndexBackend::remove()` - removes from search index
- `VectorBackend::delete()` - removes from vector index
- Audit log entry created for compliance

### Right to Access (Article 15)
Users can export their data:
```bash
subcog recall --namespace all --limit 1000 --format json
```

## Implementation Notes
- Audit log rotation: configure in `AuditConfig`
- SQLite database: use `subcog gc --purge` for permanent deletion
- Search index: rebuilds from SQLite (source of truth)
- Tombstoned memories: retained for sync, purged after retention period

## Garbage Collection

Memories from deleted branches are tombstoned (soft deleted):

```bash
# Preview stale branch cleanup
subcog gc --dry-run

# Tombstone memories from deleted branches
subcog gc

# Permanently delete old tombstoned memories
subcog gc --purge --older-than 30d
```

## Exceptions
- Legal hold: preserve data under litigation
- Compliance: maintain audit trail
