---
title: CI/CD Workflow Specification - CI (Lint & Test)
version: 1.0
date_created: 2026-08-27
last_updated: 2026-08-27
owner: DevOps Team
tags: [process, cicd, github-actions, automation, lint, test, turbo, labrinth]
---

## Workflow Overview

**Purpose**: Run monorepo lint, test, and i18n checks on push/PR/merge_group. Supports the live theses (main) and PR-driven CI with a merge-queue diff-skip optimization.

**Trigger Events**:
- Push to `main`
- `pull_request` (opened, synchronize)
- `merge_group` (checks_requested)

**Target Environments**: CI (namespace self-hosted for internal branches; `ubuntu-latest` fallback for forks/external).

## Execution Flow Diagram

```mermaid
graph TD
    A[Trigger] --> B[skip-if-clean]
    B -->|skip=true| C[Skip build]
    B -->|skip=false| D[build: Lint and Test]
    D --> E[Check out code]
    E --> F[Setup Node/Corepack]
    F --> G[Set up caches (internal only)]
    G --> H[Install apt deps]
    H --> I[Setup Rust + mold + binstall + nextest]
    I --> J[Setup cargo-sqlx]
    J --> K[Install deps + set app env]
    K --> L{check-labrinth cache}
    L -->|miss| M[Start services + setup db]
    M --> N[Lint and test]
    N --> O[Verify intl:extract]
    L -->|hit| N
    O --> P[End]

    style A fill:#e1f5fe
    style P fill:#e8f5e8
```

## Jobs & Dependencies

| Job Name | Purpose | Dependencies | Execution Context |
|----------|---------|--------------|-------------------|
| skip-if-clean | Compute whether to skip (merge_queue no-diff / internal branch) | None | `ubuntu-latest` |
| build | Lint + test + i18n checks | `skip-if-clean` (outputs `skip`, `internal`) | `namespace-profile-modrinth-turbo` if internal, else `ubuntu-latest` |

## Requirements Matrix

### Functional Requirements

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|-------------------|
| REQ-001 | Skip CI on no-diff merge queue synthetic commits | High | `skip` output true → build job skipped |
| REQ-002 | Run on internal runner when possible | Medium | `internal` output selects namespace runner |
| REQ-003 | Lint + test whole monorepo | High | `pnpm run ci` passes |
| REQ-004 | Run labrinth test harness with Redis cluster + sqlx | Medium | Cluster services up, DB setup, tests pass (**to be removed in rewrite — see Edge Cases**) |
| REQ-005 | Verify `intl:extract` committed | High | `git diff --exit-code` on locale files |

### Security Requirements

| ID | Requirement | Implementation Constraint |
|----|-------------|---------------------------|
| SEC-001 | No secrets required for CI | Only optional `secrets.GH_ACCESS_TOKEN` for merge-queue skipper |
| SEC-002 | Fork PRs run on public `ubuntu-latest` | Guarded by `internal` output |

### Performance Requirements

| ID | Metric | Target | Measurement Method |
|----|-------|--------|-------------------|
| PERF-001 | Cache reuse | rust / pnpm / apt caches (internal only) | nscloud cache action logs |
| PERF-002 | Avoid full re-lint on no-diff merge queue | skip on cache hit | merge-queue-ci-skipper output |

## Input/Output Contracts

### Inputs

```yaml
# Environment Variables (build job scope)
FORCE_COLOR: 3  # Purpose: colorized pnpm output
NEXTEST_NO_TESTS: pass  # Purpose: nextest ignores projects without tests
RUSTFLAGS: '-Dwarnings'  # Purpose: fail on warnings in CI (explicit RUSTFLAGS override; root Cargo.toml exempt during dev)
REDIS_TOPOLOGY: cluster  # Purpose: labrinth test Redis topology
REDIS_CONNECTION_TYPE: multiplexed  # Purpose: labrinth test Redis connection
REDIS_URL: 'redis://127.0.0.1:7000,...:7005'  # Purpose: 6-node cluster endpoint list (labrinth tests)
RUST_MIN_STACK: 134217728  # Purpose: avoid stack overflow in tests

# Secrets
GH_ACCESS_TOKEN: secret  # Purpose: used by local merge-queue-ci-skipper action

# Triggers
branches: [main]
pull_request types: [opened, synchronize]
merge_group types: [checks_requested]
```

### Outputs

```yaml
# Job Outputs
skip: string  # Description: skip-if-clean -> whether build job should be skipped
internal: string  # Description: skip-if-clean -> whether run is on an internal branch
# Side effects
locales_ok: check  # Description: intl:extract diff must be clean
```

### Secrets & Variables

| Type | Name | Purpose | Scope |
|------|------|---------|-------|
| Secret | GH_ACCESS_TOKEN | merge-queue-ci-skipper local action | skip-if-clean job |

## Execution Constraints

### Runtime Constraints

- **Timeout**: Default (not set)
- **Concurrency**: `${{ github.workflow }}-${{ github.ref }}`; cancel-in-progress true except `main`/`prod`
- **Resource Limits**: caches gated to internal branch runs

### Environmental Constraints

- **Runner Requirements**: Internal runner `namespace-profile-modrinth-turbo`; apt deps (cmake, libcurl4-openssl-dev, libwebkit2gtk-4.1-dev, libayatana-appindicator3-dev, librsvg2-dev) required for Rust/webkit build
- **Network Access**: GitHub, namespace cache, crates.io, npm registry, apt repos, `docker compose` (Redis cluster)
- **Permissions**: Default GITHUB_TOKEN; GH_ACCESS_TOKEN for merge queue ops

## Error Handling Strategy

| Error Type | Response | Recovery Action |
|------------|----------|-----------------|
| Lint/test fail | Build job fails | Fix source, rerun |
| Test failure on cache-HIT labrinth | Labrinth cached → no services start | Force cache miss to re-verify |
| intl:extract not run | `git diff` exits nonzero | Run `pnpm turbo run intl:extract`, commit locales |
| docker compose / sqlx fail on labrinth test | Build job fails | Check Redis cluster image and `.env.local` |

## Quality Gates

### Gate Definitions

| Gate | Criteria | Bypass Conditions |
|------|----------|-------------------|
| Code Quality (clippy/rustfmt) | `-Dwarnings`, `pnpm run ci` | None |
| i18n contract | `i18n-icu-contract prune-local --check` | None |
| Locale extraction | No diff vs committed locales | None |

## Monitoring & Observability

### Key Metrics

- **Success Rate**: PR/main CI pass rate
- **Execution Time**: Turbo cached vs cold runs
- **Resource Usage**: cache hit rate (rust/pnpm/apt)

### Alerting

| Condition | Severity | Notification Target |
|-----------|----------|-------------------|
| CI failure on main | High | Repo owners |

## Integration Points

### External Systems

| System | Integration Type | Data Exchange | SLA Requirements |
|--------|------------------|---------------|------------------|
| namespace cache | Cache | rust/pnpm/apt caches | internal only |
| Turborepo cache | Cache | `setup-turbocache` | internal only |
| labrinth test harness | Test infrastructure | docker compose `clustered-redis`, sqlx-cli, `.env.local` | **CURRENT — to be removed in Task 11 rewrite** |

### Dependent Workflows

| Workflow | Relationship | Trigger Mechanism |
|----------|--------------|-------------------|
| merge-queue-ci-skipper (local action) | Skip optimization | `.github/merge-queue-ci-skipper` invoked from skip-if-clean |

## Compliance & Governance

### Audit Requirements

- **Execution Logs**: GitHub Actions run logs
- **Approval Gates**: Branch protection on main
- **Change Control**: PR review

### Security Controls

- **Access Control**: Fork PRs on public runners; internal branch runs use namespace
- **Secret Management**: GH_ACCESS_TOKEN scoped minimal
- **Vulnerability Scanning**: None in-workflow

## Edge Cases & Exceptions

### Scenario Matrix

| Scenario | Expected Behavior | Validation Method |
|----------|-------------------|-------------------|
| Merge queue with no diff | `skip=true` → job ultimately skipped | Trigger merge_group |
| Fork/external PR | Runs on `ubuntu-latest`, no namespace cache/turbocache | Observe runner label |
| Labrinth tests cache-HIT | `check-labrinth` sets `needs_services=false`; no docker/sqlx steps | Dry-run test filter, observe cache status |
| i18n locale drift | `intl:extract --force` then diff fails CI | Remove a translation, run workflow |
| **Labrinth harness removal** | **KNOWN SCOPE NOTE:** the labrinth test steps (check-labrinth, docker compose `clustered-redis`, `sqlx database setup`) are documented as CURRENT; the CI rewrite (Task 11) is expected to remove them since labrinth is no longer in-scope. Recorded as snapshot, not a defect. | Compare against target rewrite PR |

## Validation Criteria

### Workflow Validation

- **VLD-001**: `skip`/`internal` outputs correctly gate the build job
- **VLD-002**: `pnpm run ci` exits zero on healthy repo
- **VLD-003**: `git diff` on locales/index.json is clean after intl:extract
- **VLD-004**: labrinth tests run with the documented Redis cluster config (until removed)

### Performance Benchmarks

- **PERF-001**: Turbo/nscloud cache Hit on repeated PRs
- **PERF-002**: No CI on no-diff merge queue synthetic commits

## Change Management

### Update Process

1. **Specification Update**: Modify this document first
2. **Review & Approval**: PR review (DevOps)
3. **Implementation**: Apply changes to workflow
4. **Testing**: Trigger push/PR/merge_group
5. **Deployment**: Merge to main

### Version History

| Version | Date | Changes | Author |
|---------|------|---------|--------|
| 1.0 | 2026-08-27 | Initial specification (documents pre-rewrite `turbo-ci.yml`) | DevOps Team |

## Related Specifications

- [spec-process-cicd-app-build.md](./spec-process-cicd-app-build.md)
- [spec-process-cicd-app-release.md](./spec-process-cicd-app-release.md)
