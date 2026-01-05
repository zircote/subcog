# Production Deployment Plan

This plan defines the standard production deployment workflow for Subcog.

## Preconditions

- Release artifact built and verified (`make ci` passed).
- Configuration reviewed for the target environment.
- Database backup completed (if using SQLite/PostgreSQL).
- Monitoring/alerting configured (see `MONITORING_ALERTING.md`).

## Deployment Steps

1. **Build and verify**
   - Build the release binary (`cargo build --release` or CI artifact).
   - Verify checksums/signatures for the release artifact.

2. **Configure environment**
   - Set required environment variables (see `environment-variables.md`).
   - Confirm `SUBCOG_ORG_SCOPE_ENABLED` matches the deployment scope.

3. **Database readiness**
   - Take a backup of the SQLite/PostgreSQL database.
   - Run a smoke capture/recall to ensure migrations apply cleanly.

4. **Deploy**
   - Roll out the binary/container to the target hosts.
   - Restart the service or CLI wrapper as appropriate.

5. **Post-deploy validation**
   - `subcog status` for basic health checks.
   - Run a short capture/recall workflow.
   - Confirm logs, metrics, and traces are flowing.

## Rollback

- Restore the database backup if migrations were applied.
- Roll back the binary/container to the previous release.
- Re-run `subcog status` and smoke tests.

## Verification Checklist

- [ ] Release artifact verified
- [ ] Config/env reviewed
- [ ] Backup completed
- [ ] Smoke tests passed
