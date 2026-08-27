# 仓库配置：Secrets 与 Variables

本仓库的 GitHub Actions 依赖一组仓库级 **Secrets**（加密）与 **Variables**（明文）。配置位置：GitHub 仓库 → **Settings → Secrets and variables → Actions**。

> 所有值均为**可选**：缺少签名/存储/Crowdin 相关配置时 workflow 不会失败，而是降级（未签名构建、跳过更新 manifest 等）。只有 `i18n-pull.yml` 在缺少 Crowdin 配置时会在 preflight 阶段报错退出。

## 一、Secrets（敏感，加密存储）

| Secret | 用途 | 缺失时的行为 | 用到的工作流 |
| ------ | ------ | ------ | ------ |
| `TAURI_PRIVATE_KEY` | Tauri updater 签名私钥（`.sig` 生成/验证） | updater 产物缺失 → release 跳过 manifest/S3 | theseus-build, theseus-release |
| `TAURI_KEY_PASSWORD` | 上述私钥密码 | 同上 | theseus-build, theseus-release |
| `APPLE_CERTIFICATE` | macOS 签名证书（base64） | macOS 未签名 | theseus-build |
| `APPLE_CERTIFICATE_PASSWORD` | 证书密码 | 同上 | theseus-build |
| `APPLE_SIGNING_IDENTITY` | 签名身份（如 `Developer ID Application: ...`） | 同上 | theseus-build |
| `APPLE_ID` / `APPLE_PASSWORD` | Apple ID + App 专用密码（公证） | 跳过公证 | theseus-build |
| `APPLE_TEAM_ID` | Apple Team ID | 同上 | theseus-build |
| `DIGICERT_ONE_SIGNER_API_KEY` | DigiCert ONE Signer API Key（Windows 联署） | Windows 未签名 | theseus-build |
| `DIGICERT_ONE_SIGNER_CLIENT_CERTIFICATE_BASE64` | DigiCert 签名客户端证书（base64/p12） | 同上 | theseus-build |
| `DIGICERT_ONE_SIGNER_CLIENT_CERTIFICATE_PASSWORD` | 客户端证书密码 | 同上 | theseus-build |
| `LAUNCHER_FILES_BUCKET_ACCESS_KEY_ID` | 更新文件存储（R2/S3）访问密钥 | 不上传更新文件，仅 GitHub Release | theseus-release |
| `LAUNCHER_FILES_BUCKET_SECRET_ACCESS_KEY` | 更新文件存储访问密钥 | 同上 | theseus-release |
| `CROWDIN_PERSONAL_TOKEN` | Crowdin 个人访问令牌（拉取翻译） | `i18n-pull.yml` preflight **失败** | i18n-pull |

## 二、Variables（非敏感，明文）

| Variable | 用途 | 缺失时的行为 | 用到的工作流 |
| ------ | ------ | ------ | ------ |
| `CROWDIN_PROJECT_ID` | Crowdin 项目 ID | `i18n-pull.yml` preflight **失败** | i18n-pull |
| `LAUNCHER_FILES_BUCKET_NAME` | 更新文件桶名（如 `packrinth-updates`） | 不上传更新文件 | theseus-release |
| `LAUNCHER_FILES_BUCKET_REGION` | 桶区域 | 同上 | theseus-release |
| `LAUNCHER_FILES_BUCKET_ENDPOINT_URL` | S3 兼容端点（Cloudflare R2 必填） | 同上 | theseus-release |
| `LAUNCHER_FILES_BUCKET_BASE_URL` | 更新服务基础 URL（如 `https://updates.example.com`），写入 `updates.json` | 生成 manifest 时 URL 为空（updater 不可用） | theseus-release |

## 三、解释与要点

### 签名（`SIGNED=yes` 的判定）

`theseus-build.yml` 的 `Configure signing` 步骤按以下规则判定是否签名：

```
签名请求（tag-type=release 或 sign-windows-binaries=true）
  AND DIGICERT_ONE_SIGNER_API_KEY 存在   → Windows 签名，bundles=nsis,updater
  AND APPLE_CERTIFICATE 存在              → macOS 签名（ENABLE_CODE_SIGNING=true）
  AND TAURI_PRIVATE_KEY 存在              → updater .sig 产物
```

任一缺失 → 本次构建自动降级为未签名（不会失败）。已签名构建各平台产物必须带 `.sig`/`.sig.zip`，release 才会生成 `updates.json` 并上传更新服务器。

### 更新服务器（发布上线前置条件）

要让用户能自动更新，发布前需一次性完成：

1. 建 S3 兼容桶（推荐 Cloudflare R2），记下桶名/区域/端点。
2. 配置 Variables：`LAUNCHER_FILES_BUCKET_NAME` / `_REGION` / `_ENDPOINT_URL` / `_BASE_URL`。
3. 配置 Secrets：`LAUNCHER_FILES_BUCKET_ACCESS_KEY_ID` / `_SECRET_ACCESS_KEY`（桶只读权限仅放 workspace 内）。
4. **首次发布前**将 `updates.json` 上传到 `{LAUNCHER_FILES_BUCKET_BASE_URL}/updates.json`（release workflow 会在发布时同时上传）。
5. `apps/app/tauri-release.conf.json` 的 `plugins.updater.endpoints` 目前为空数组——将 `LAUNCHER_FILES_BUCKET_BASE_URL` 填入（客户端的端点配置，runtimes 中已嵌入 pubkey）。

### Crowdin（翻译拉取）

- `vars.CROWDIN_PROJECT_ID` + `secrets.CROWDIN_PERSONAL_TOKEN` 必填，否则 `i18n-pull.yml`（每周一自动）preflight 退出。
- 只拉取不推送（push workflow 已移除）；Packrinth 自有文案（`locales-packrinth/`）不走 Crowdin。

### 其他

- `GITHUB_TOKEN` 是自动注入的，不需要配置。
- 无需 `GH_ACCESS_TOKEN`（`merge-queue-ci-skipper` 已删除）。
- Windows 签名需要 `choco install jsign`（CI 自动）与 Java 17（`setup-java` 步骤自动配置）。
- 若使用 Fork 仓库管理：Fork 中 Secrets 默认不继承，需要自己在 fork 仓库重新配置（拉取请求在 fork 上跑即降级为未签名）。

## 四、快速检查清单

| 目标 | 必填项 |
| ------ | ------ |
| 仅开发/本地 | 无 |
| CI 正常通过 | 无（turbo-ci/check-* 不用 secrets） |
| 翻译同步 | `CROWDIN_PROJECT_ID`, `CROWDIN_PERSONAL_TOKEN` |
| 签名发布（macOS） | `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` |
| 签名发布（Windows） | `DIGICERT_ONE_SIGNER_API_KEY`, `DIGICERT_ONE_SIGNER_CLIENT_CERTIFICATE_BASE64`, `DIGICERT_ONE_SIGNER_CLIENT_CERTIFICATE_PASSWORD` |
| 更新器（updater） | `TAURI_PRIVATE_KEY`, `TAURI_KEY_PASSWORD`, `LAUNCHER_FILES_BUCKET_*`（4 个 Variables + 2 个 Secrets），且 `endpoints` 已配置 |
| 发布无需签名 | 只需 tag + 手动 dispatch（release 跳过 manifest 上传，仅发 GitHub Release） |
