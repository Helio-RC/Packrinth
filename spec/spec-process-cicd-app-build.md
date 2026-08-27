---
title: CI/CD Workflow Specification - App Build
version: 1.0
date_created: 2026-08-27
last_updated: 2026-08-27
owner: DevOps Team
tags: [process, cicd, github-actions, automation, tauri, desktop-build]
---

## Workflow Overview

**Purpose**: Build the Packrinth App desktop application (Linux, macOS, Windows) as signed/unsigned installers and app bundles, and upload them as GitHub Actions artifacts for downstream consumption by the release workflow.

**Trigger Events**:
- Push to `main` branches
- `v*` tag pushes
- Manual `workflow_dispatch` (with `sign-windows-binaries`, `environment`, `app-version-override` inputs)
- Path filters restrict auto-triggering to app/frontend/lib/assets/ui/utils sources and this workflow file.

**Target Environments**: `prod`, `staging`, `prod-with-staging-archon` (selected via `environment` input; used to pick `.env` template). `PRODUCT_NAME` is `Packrinth`, declared once at workflow scope and reused by every step.

## Execution Flow Diagram

```mermaid
graph TD
    A[Trigger] --> B[Build job - macOS]
    A --> C[Build job - Windows]
    A --> D[Build job - Linux]
    B --> E[Upload artifact: App bundle (universal-apple-darwin)]
    C --> F[Upload artifact: App bundle (x86_64-pc-windows-msvc)]
    D --> G[Upload artifact: App bundle (x86_64-unknown-linux-gnu)]

    style A fill:#e1f5fe
    style E fill:#e8f5e8
    style F fill:#e8f5e8
    style G fill:#e8f5e8
```

## Jobs & Dependencies

| Job Name | Purpose | Dependencies | Execution Context |
|----------|---------|--------------|-------------------|
| build | Compile + bundle the app for each matrix platform | None (single job, matrix expansion) | matrix runners `macos-latest`/`windows-latest`/`ubuntu-latest` |

## Requirements Matrix

### Functional Requirements

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|-------------------|
| REQ-001 | Build each of the 3 matrix platforms | High | All 3 matrix legs succeed |
| REQ-002 | Inject correct app version into manifests | High | `package.version` set in both Cargo.tomls and package.json |
| REQ-003 | Select dev vs release Tauri config | High | Release conf used on main/tags/override; dev conf else |
| REQ-004 | Upload app bundles per platform | High | Artifact named `App bundle (<target>)` per platform |
| REQ-005 | Strip dev git hash into product name | Medium | `tauri-dev.conf.json` generated with `productName`/`identifier` |

### Security Requirements

| ID | Requirement | Implementation Constraint |
|----|-------------|---------------------------|
| SEC-001 | Signing keys never exposed in logs | Secrets passed only via `env`; `signer-client-cert.p12` created transiently and removed afterward |

### Performance Requirements

| ID | Metric | Target | Measurement Method |
|----|-------|--------|-------------------|
| PERF-001 | Rust compile time | Minimized via sccache | sccache cache hit ratio |
| PERF-002 | Build time on repeated runs | Reduced via `actions/cache` (cargo + pnpm) | Cache action logs |

## Input/Output Contracts

### Inputs

```yaml
# Environment Variables (build job scope)
VITE_STRIPE_PUBLISHABLE_KEY: secret  # Purpose: Stripe publishable key baked at build time (pk_live_<key>)

# Manual inputs (workflow_dispatch)
sign-windows-binaries: boolean  # Purpose: force Windows code signing (default false)
environment: choice              # Purpose: prod | staging | prod-with-staging-archon (default prod)
app-version-override: string     # Purpose: for updater testing; overrides git-derived version

# Repository Triggers
paths: [.github/workflows/theseus-build.yml, apps/app/**, apps/app-frontend/**,
        packages/app-lib/**, packages/assets/**,
        packages/ui/**, packages/utils/**]
branches: [main]
tags: ['v*']
```

### Outputs

```yaml
# Job Outputs
build_artifacts: artifact  # Description: named per target, collected downstream by release workflow
```

### Secrets & Variables

| Type | Name | Purpose | Scope |
|------|------|---------|-------|
| Secret | APPLE_CERTIFICATE | macOS signing (also gates signing via ENABLE_CODE_SIGNING) | macos leg |
| Secret | APPLE_CERTIFICATE_PASSWORD | macOS signing cert password | macos leg |
| Secret | APPLE_SIGNING_IDENTITY | macOS signing identity | macos leg |
| Secret | APPLE_ID | macOS notarization Apple ID | macos leg |
| Secret | APPLE_TEAM_ID | macOS Apple team | macos leg |
| Secret | APPLE_PASSWORD | macOS notarization password | macos leg |
| Secret | TAURI_PRIVATE_KEY | App updater signing private key (all legs) | all legs |
| Secret | TAURI_KEY_PASSWORD | App updater signing key password (all legs) | all legs |
| Secret | DIGICERT_ONE_SIGNER_API_KEY | Windows code signing API key | windows leg |
| Secret | DIGICERT_ONE_SIGNER_CLIENT_CERTIFICATE_BASE64 | Windows signer client cert (base64) | windows leg |
| Secret | DIGICERT_ONE_SIGNER_CLIENT_CERTIFICATE_PASSWORD | Windows signer client cert password | windows leg |

## Execution Constraints

### Runtime Constraints

- **Timeout**: Default GitHub Action job timeout (none set explicitly in workflow)
- **Concurrency**: `concurrency` group `${{ github.workflow }}-${{ github.ref }}`; `cancel-in-progress: true` except on `main`/`prod`
- **Resource Limits**: `actions/cache` for `cargo` and `pnpm`; sccache for Rust; per-platform Java 17 for Windows signing

### Environmental Constraints

- **Runner Requirements**: Linux requires `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` via apt; Windows requires Java 17 (`JAVA_HOME_17_X64`) and optional `jsign` via choco; macOS uses universal `x86_64-apple-darwin` target
- **Network Access**: GitHub, npm/pnpm registry, crates.io, apt repos, choco, dasel gh-release, Stripe (no runtime API, key embed only)
- **Permissions**: Default GITHUB_TOKEN scope; secrets as listed above

## Error Handling Strategy

| Error Type | Response | Recovery Action |
|------------|----------|-----------------|
| Build Failure | Job fails (fail-fast: false keeps other legs running) | Inspect per-leg logs; rerun workflow |
| Windows signing missing (dev) | `dasel delete` removes `bundle.windows.signCommand` | Run without signing |
| Missing environment template | `cp` fails at `Set application version` step | Ensure `.env.<environment>` exists in app-lib |

## Quality Gates

### Gate Definitions

| Gate | Criteria | Bypass Conditions |
|------|----------|-------------------|
| Platform build | All 3 legs pass | Manual workflow_dispatch may shorten to staging/dev conf |
| Artifact presence | Upload `App bundle (<target>)` per platform | Fail if no bundle globs match |
| Signing | Signing only on tags/main/override | Dev builds unsigned; Windows signing skippable via input |

## Monitoring & Observability

### Key Metrics

- **Success Rate**: All matrix legs green per run
- **Execution Time**: Tracked via `actions/cache` health + sccache hit ratio

### Alerting

| Condition | Severity | Notification Target |
|-----------|----------|-------------------|
| Build leg failure | High | GitHub Actions run failure (repo owners) |

## Integration Points

### External Systems

| System | Integration Type | Data Exchange | SLA Requirements |
|--------|------------------|---------------|------------------|
| GitHub Actions cache | Cache | cargo + pnpm caches via `actions/cache` | Cache keyed per lockfile |
| tauri bundler | Build | Installer packages (AppImage/deb/rpm/dmg/app.tar.gz/nsis) | Deterministic artifact names |
| GitHub Actions artifacts | Output | App bundle per target | Downstream consumed by release workflow |

### Dependent Workflows

| Workflow | Relationship | Trigger Mechanism |
|----------|--------------|-------------------|
| Packrinth App release (`theseus-release`) | Downstream consumer (workflow_run) | Fires on the `Packrinth App build` run completing |

## Compliance & Governance

### Audit Requirements

- **Execution Logs**: GitHub Actions run logs (retained per repo policy)
- **Approval Gates**: None for build; gated implicitly by branch/tag push
- **Change Control**: Workflow changes require PR review

### Security Controls

- **Access Control**: Secrets scoped to build job env only
- **Secret Management**: Rotated outside repo; shared cert/base64 Windows signer material
- **Vulnerability Scanning**: None in-workflow (out of scope; handled elsewhere)

## Edge Cases & Exceptions

### Scenario Matrix

| Scenario | Expected Behavior | Validation Method |
|----------|-------------------|-------------------|
| Dev build (non-main, non-tag) | Uses `tauri-dev.conf.json`, no code signing, dev git-hash product naming | Run on feature branch via workflow_dispatch |
| app-version-override set | Forces release conf + uses override string as version; git-hash regex not applied | Pass override and inspect Cargo.toml/package.json |
| Windows dev build | `signCommand` deleted to skip signing | Inspect generated `tauri-release.conf.json` |
| Non-`v` tag push | Workflow runs but uses git-describe-derived canary version | Push `main` PR; verify version suffix |
| Concurrency cancel | Non-main/prod runs cancel in-flight jobs for same ref | Trigger overlapping runs on a PR |

## Validation Criteria

### Workflow Validation

- **VLD-001**: All three platform legs produce `App bundle (<target>)` artifacts with expected globset
- **VLD-002**: Version manifest (Cargo.toml ×2 + package.json) matches `VERSION_TAG` minus leading `v`
- **VLD-003**: `tauri-dev.conf.json` contains git-hash-derived product name when dev
- **VLD-004**: Release conf selected exactly when `refs/heads/main`, `refs/tags/v*`, or override set

### Performance Benchmarks

- **PERF-001**: sccache/`actions/cache` hit on repeat runs
- **PERF-002**: No redundant pnpm/crate re-download when cache warm

## Change Management

### Update Process

1. **Specification Update**: Modify this document first
2. **Review & Approval**: PR review (DevOps)
3. **Implementation**: Apply changes to workflow
4. **Testing**: Dry-run workflow_dispatch on staging input
5. **Deployment**: Merge to main

### Version History

| Version | Date | Changes | Author |
|---------|------|---------|--------|
| 1.0 | 2026-08-27 | Initial specification (documents rewritten `theseus-build.yml`) | DevOps Team |

## Related Specifications

- [spec-process-cicd-app-release.md](./spec-process-cicd-app-release.md)
- [spec-process-cicd-ci.md](./spec-process-cicd-ci.md)
