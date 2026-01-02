# Change Control Process (COMP-H6)

## Purpose
Define procedures for managing changes to the subcog system.

## Change Classification

| Type | Risk | Approval | Examples |
|------|------|----------|----------|
| Emergency | Critical | Post-hoc | Security patch, outage fix |
| Major | High | CAB | Schema change, new backend |
| Standard | Medium | Peer review | Feature, bug fix |
| Minor | Low | Auto-approve | Docs, typos |

## Process

### 1. Request
- Create issue/ticket with change details
- Impact assessment
- Rollback plan

### 2. Review
- Peer code review required
- CI must pass: `make ci`
- Security review for auth/crypto changes

### 3. Approval
- Standard: 1 approving review
- Major: CAB or designated approver
- Emergency: Document post-deployment

### 4. Deployment
- Use feature branches
- Merge to main via PR
- Tag releases per semver

### 5. Verification
- Run integration tests
- Monitor metrics/logs
- Confirm rollback capability

## CI/CD Gates
All changes must pass:
- `cargo fmt -- --check`
- `cargo clippy --all-targets --all-features`
- `cargo test --all-features`
- `cargo doc --no-deps`
- `cargo deny check`

## Rollback Procedures
- Git: `git revert <commit>`
- Database: Restore from backup
- Config: Redeploy previous version
