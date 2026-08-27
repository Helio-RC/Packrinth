---
title: CI/CD Workflow Specification - i18n Push
version: 1.0
date_created: 2026-08-27
last_updated: 2026-08-27
owner: DevOps Team
tags: [process, cicd, github-actions, automation, i18n, crowdin, localization]
---

## Workflow Overview

**Purpose**: Upload source locale files to Crowdin when English sources change, and clear stale ICU translations in Crowdin.

**Trigger Events**:
- Push to `main` with path filters (workflow file, `en-US` locales, i18n scripts, package/lock manifests, `crowdin.yml`)
- `workflow_dispatch`

**Target Environments**: `main` branch only (job guarded by `github.ref == 'refs/heads/main'`).

## Execution Flow Diagram

```mermaid
graph TD
    A[Trigger] --> B[Preflight check]
    B -->|fail| C[Abort]
    B -->|pass| D[Checkout (depth 2)]
    D --> E[Setup Node + Corepack]
    E --> F[Install script deps]
    F --> G[Query branch name]
    G --> H[Upload translations to Crowdin]
    H --> I[Clear stale ICU translations in Crowdin]
    I --> J[End]

    style A fill:#e1f5fe
    style J fill:#e8f5e8
```

## Jobs & Dependencies

| Job Name | Purpose | Dependencies | Execution Context |
|----------|---------|--------------|-------------------|
| push_translations | Upload sources to Crowdin | None | `ubuntu-latest` |

## Requirements Matrix

### Functional Requirements

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|-------------------|
| REQ-001 | Preflight-verify Crowdin creds | High | Abort if project id or token missing |
| REQ-002 | Upload English sources to Crowdin | High | `upload_sources=true`; downloads/push/PR disabled |
| REQ-003 | Clear stale ICU translations | High | `i18n-icu-contract clear-crowdin-changed` against `HEAD^` |

### Security Requirements

| ID | Requirement | Implementation Constraint |
|----|-------------|---------------------------|
| SEC-001 | Crowdin token not logged | Passed via `env` (workflow-level secret) |
| SEC-002 | Only push from main | Job `if` guard |

### Performance Requirements

| ID | Metric | Target | Measurement Method |
|----|-------|--------|-------------------|
| PERF-001 | On-demand sync | Only when sources change | Path filters |

## Input/Output Contracts

### Inputs

```yaml
# Environment Variables
CROWDIN_PROJECT_ID: variable (vars)  # Purpose: Crowdin project id (preflight + upload + clear)
CROWDIN_PERSONAL_TOKEN: secret  # Purpose: Crowdin personal access token (preflight + upload + clear)
crowdin_branch_name: string  # Purpose: '[<owner>.<repo>] <safe_branch>'

# Triggers
branches: [main]
paths: ['.github/workflows/i18n-push.yml', 'apps/*/src/locales/en-US/**', 'apps/*/locales/en-US/**',
        'packages/*/src/locales/en-US/**', 'packages/*/locales/en-US/**',
        'scripts/i18n-icu-contract.ts', 'package.json', 'pnpm-lock.yaml', 'crowdin.yml']
```

### Outputs

```yaml
# Side effects
crowdin_sources: file  # Description: English sources synced to Crowdin
crowdin_cleanup: check  # Description: stale ICU translations cleared
```

### Secrets & Variables

| Type | Name | Purpose | Scope |
|------|------|---------|-------|
| Variable | CROWDIN_PROJECT_ID | Crowdin project ID | workflow |
| Secret | CROWDIN_PERSONAL_TOKEN | Crowdin API token | workflow |

## Execution Constraints

### Runtime Constraints

- **Timeout**: Default
- **Concurrency**: workflow `i18n-management` + job group `i18n-push:<ref>` (cancel-in-progress)
- **Resource Limits**: Minimal

### Environmental Constraints

- **Runner Requirements**: `ubuntu-latest`; Node via `.nvmrc`; `fetch-depth: 2` (for HEAD^ base ref)
- **Network Access**: GitHub, npm registry, Crowdin API
- **Permissions**: Default GITHUB_TOKEN

## Error Handling Strategy

| Error Type | Response | Recovery Action |
|------------|----------|-----------------|
| Missing Crowdin creds | Preflight abort (`exit 1`) | Configure vars/secrets |
| Upload failure | Step fails | Retry workflow_dispatch |

## Quality Gates

### Gate Definitions

| Gate | Criteria | Bypass Conditions |
|------|----------|-------------------|
| Credentials | Both `CROWDIN_PROJECT_ID` and `CROWDIN_PERSONAL_TOKEN` defined | None |
| ICU contract | `clear-crowdin-changed` against `HEAD^` | None |

## Monitoring & Observability

### Key Metrics

- **Success Rate**: source sync complete
- **Execution Time**: minutes

### Alerting

| Condition | Severity | Notification Target |
|-----------|----------|-------------------|
| Push failure | Medium | Repo owners |

## Integration Points

### External Systems

| System | Integration Type | Data Exchange | SLA Requirements |
|--------|------------------|---------------|------------------|
| Crowdin | Upload | source files | On source change |

### Dependent Workflows

| Workflow | Relationship | Trigger Mechanism |
|----------|--------------|-------------------|
| i18n-pull (Crowdin pull) | Peer (shared i18n-management concurrency) | Sibling workflow — mutually exclusive via concurrency group |

## Compliance & Governance

### Audit Requirements

- **Execution Logs**: GitHub Actions run logs
- **Approval Gates**: merged to main triggers sync
- **Change Control**: PR review

### Security Controls

- **Access Control**: default token
- **Secret Management**: token rotates via repo secrets
- **Vulnerability Scanning**: N/A

## Edge Cases & Exceptions

### Scenario Matrix

| Scenario | Expected Behavior | Validation Method |
|----------|-------------------|-------------------|
| Job dispatched on non-main ref | Guarded `if` skips | Trigger on a branch |
| Missing project id/token | Preflight exits 1 | Unset one var/secret |
| Non-English-source change | Path filters prevent trigger | Change a file outside filters |

## Validation Criteria

### Workflow Validation

- **VLD-001**: `upload_sources=true`; download/push/create_pull_request disabled
- **VLD-002**: path filter matches `en-US` locale files and i18n manifest
- **VLD-003**: `clear-crowdin-changed --base-ref HEAD^` runs with crowdin branch name

### Performance Benchmarks

- **PERF-001**: Trigger only when relevant paths change

## Change Management

### Update Process

1. **Specification Update**: Modify this document first
2. **Review & Approval**: PR review
3. **Implementation**: Apply changes to workflow
4. **Testing**: workflow_dispatch dry run
5. **Deployment**: Merge to main

### Version History

| Version | Date | Changes | Author |
|---------|------|---------|--------|
| 1.0 | 2026-08-27 | Initial specification (documents `i18n-push.yml`) | DevOps Team |

## Related Specifications

- [spec-process-cicd-i18n-pull.md](./spec-process-cicd-i18n-pull.md)
