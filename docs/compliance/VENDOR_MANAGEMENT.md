# Vendor Management (COMP-H5)

## Purpose
Define procedures for managing third-party vendors and dependencies.

## Vendor Classification

| Category | Risk Level | Review Frequency | Examples |
|----------|------------|------------------|----------|
| Data Processors | High | Annual | LLM providers, cloud storage |
| Infrastructure | High | Annual | Cloud platforms, CDNs |
| Development Tools | Medium | Semi-annual | CI/CD, monitoring |
| Libraries | Medium | Per-release | Cargo dependencies |

## Assessment Criteria
- [ ] SOC 2 Type II certification
- [ ] GDPR compliance (for EU data)
- [ ] Security questionnaire completed
- [ ] Data processing agreement signed
- [ ] Incident notification SLA defined

## Dependency Management

### Supply Chain Security
- `cargo deny check` enforces:
  - No known vulnerabilities (advisories)
  - Approved licenses only (MIT, Apache-2.0, BSD)
  - Banned crate detection
  - Source verification (crates.io only)

### Current Dependencies
See `Cargo.toml` for full list. Key vendors:
- LLM: OpenAI, Anthropic, Ollama, LM Studio
- Database: PostgreSQL, SQLite, Redis
- Embedding: FastEmbed (local)

## Review Process
1. Assess vendor against criteria
2. Document risk assessment
3. Obtain required agreements
4. Add to approved vendor list
5. Monitor vendor security bulletins
