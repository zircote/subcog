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
- `PersistenceBackend::delete()` - removes from git notes
- `IndexBackend::remove()` - removes from search index
- Audit log entry created for compliance

### Right to Access (Article 15)
Users can export their data:
```bash
subcog recall --namespace all --limit 1000 --format json
```

## Implementation Notes
- Audit log rotation: configure in `AuditConfig`
- Git notes: consider periodic gc for orphaned notes
- Search index: rebuilds from git notes (source of truth)

## Exceptions
- Legal hold: preserve data under litigation
- Compliance: maintain audit trail
