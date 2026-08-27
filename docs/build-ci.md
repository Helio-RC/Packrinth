# 构建流程（手动与 Actions）

## 前置条件

- Node.js ≥ 24.15.0（`.nvmrc`）
- pnpm 10（`corepack enable` 后使用 `packageManager: pnpm@10.33.2`）
- Rust 1.95.0（`rust-toolchain.toml`）
- Tauri 2 系统依赖：
  - Linux：`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libcurl4-openssl-dev cmake`（构建 rpm/deb 需 `cargo-deb`, `linuxdeploy` 等由 tauri 处理或额外安装）
  - macOS：Xcode CLI tools；打通用包需要 `rustup target add x86_64-apple-darwin`
  - Windows：MSVC toolchain + WiX/NSIS（tauri 自动获取）

## 依赖安装

```bash
pnpm install
```

- 有补丁：`patches/readable-stream@2.3.8.patch`
- `minimumReleaseAge: 10080`（pnpm-workspace.yaml，只用 ≥7 天的发布版，防止依赖被撤包）

## 环境文件

构建前选择环境：

```bash
cp packages/app-lib/.env.staging packages/app-lib/.env   # 开发/CI
# 或
cp packages/app-lib/.env.prod packages/app-lib/.env       # 生产
```

`apps/app/Cargo.toml` 的版本在 CI 中由 dasel 覆盖为 tag 版本；本地保持 `1.0.0-local` 即可。

## A. 手动构建

### 开发运行

```bash
pnpm app:dev
```

### 前端独立开发（不经 Tauri）

```bash
pnpm --filter @modrinth/app-frontend dev
```

### 生产打包（默认配置）

```bash
pnpm app:build                # = tauri build（使用 apps/app/tauri.conf.json）
```

### 仿 CI 的打包（指定 config）

```bash
# release 配置（与 CI 相同；tauri-release.conf.json 含签名/更新器插件配置）
pnpm --filter @modrinth/app run tauri build --config tauri-release.conf.json
# dev 配置（CI 为开发提交生成 tauri-dev.conf.json 指向 productName=Packrinth (dev-<sha>)）
pnpm --filter @modrinth/app run tauri build --config tauri-dev.conf.json
```

- macOS 通用二进制：`--target universal-apple-darwin`（CI 在 main/tag 上使用）
- Windows 只打 NSIS+updater bundles：`--bundles "nsis,updater"`（CI 做法）

### 产出物位置

- `target/release/bundle/{appimage,deb,rpm,nsis}/`
- `target/universal-apple-darwin/release/bundle/{macos,dmg}/`

## B. GitHub Actions 构建

### 触发方式

| 事件 | 触发的 workflow | 说明 |
| ------ | ------ | ------ |
| PR → `main` | theseus-build, turbo-ci, check-rust, check-generic | PR 门禁：构建 dev 配置（不签名、dev productName），lint + test |
| 手工 dispatch 构建 | theseus-build（分支/tag、tag-type、release-tag-suffix、环境、Windows 签名开关） | dev 或 release 构建；release 会再自动触发 theseus-release |
| 手工 dispatch 发布 | theseus-release（version + build-run-id） | 消费已成功构建的 artifacts，生成 updates.json、发 GitHub Release |
| 定时（周一） | i18n-pull（Crowdin 拉取翻译） | 翻译同步，无 push |

### 版本设定（重点）

**版本来源 = git tag（`vX.Y.Z`）为真源**，通过 `git describe` 派生；`release-tag-suffix` 输入可追加预发布后缀。

| 场景 | version 输出示例 |
| ------ | ------ |
| 开发分支（无 tag 命中） | `0.5.0-canary+12.gabc123` |
| tag `v0.5.0` 构建 | `0.5.0` |
| tag `v0.5.0` + suffix `-beta.1` | `0.5.0-beta.1` |

规则（`Set application version and environment` 步骤）：

```bash
APP_VERSION="$(git describe --tags --always)"
if [ -n "$TAG_SUFFIX" ]; then
  APP_VERSION="${APP_VERSION}${TAG_SUFFIX}"        # 直接追加，如 v0.5.0-beta.1
else
  APP_VERSION="$(echo "$APP_VERSION" | sed -E 's/-([0-9]+)-(g[0-9a-fA-F]+)$/-canary+\1.\2/')"
fi
APP_VERSION="${APP_VERSION#v}"                     # 去掉前导 v 写入 manifest
```

- 三处写入：`apps/app/Cargo.toml`、`packages/app-lib/Cargo.toml`、`apps/app-frontend/package.json`（dasel）。
- **正式发布推荐流程**：
  1. 在目标提交上打 tag：`git tag v0.5.0 && git push origin v0.5.0`
  2. 手动运行 **Packrinth App build**，输入 `branch: v0.5.0`、`tag-type: release`（suffix 留空）
  3. 构建成功后自动触发 **Packrinth App release**（带 version 与 run-id）
  4. 也可以手动运行 release workflow 重新发布（填 `version: 0.5.0` 和 build run id）

### theseus-build.yml 步骤摘要

1. checkout（fetch-depth 0；`ref` 来自 `branch` 输入）→ `Configure signing` 探测各签名密钥存在性（`SIGNED=yes/no`）→ rust toolchain → mold(sccache 包装) → 缓存 cargo/pnpm。
2. 生成 `apps/app/tauri-dev.conf.json`：
   - `productName/mainBinaryName = "Packrinth (dev-<git hash>)"`，`identifier = "PackrinthApp-dev-<git hash>"`（identifier 里含空格，需确认 Windows 兼容；这是上游实现）。
3. 安装系统依赖（Linux：webkit2gtk-4.1 等；Java 17；dasel）。
4. **版本与环境注入**（见上方「版本设定」）；`cp packages/app-lib/.env.<environment> packages/app-lib/.env`（environment 默认 prod）。
5. `pnpm install`
6. 签名与配置修正：`SIGNED=yes` 时 Windows 经 choco 安装 jsign；否则 `dasel delete` 掉 `bundle.windows.signCommand`、`bundle.createUpdaterArtifacts`、`build.features`，并跳过 updater bundles——**构建不因缺签名而失败**。
7. 分平台构建（macOS universal / Linux / Windows），注入 Tauri 签名密钥、Apple 公证与 DigiCert 凭证（仅当存在）。
8. upload-artifact 收集 AppImage/deb/rpm/nsis/app.tar.gz/dmg 与 `.sig`/`.sig.zip` 等。
9. `tag-type=release` 时经 `gh workflow run` 自动触发 release workflow。

### theseus-release.yml 步骤摘要

手工 dispatch（`version` + `build-run-id`；由 build 自动触发或手动运行）：

1. checkout 到 `v<version>`；校验 tag 存在且 SHA 匹配。
2. 按 `run_id` 下载 artifacts（dawidd6/action-download-artifact）。
3. 检查各平台 `.sig`（无签名时跳过 manifest/S3 并告警）。
4. `npx tsx scripts/build-theseus-release-notes.ts` 生成 release-notes.md。
5. jq 组装 `updates.json`（Tauri updater manifest，含各平台 signature + URL）。
6. aws s3 cp 上传各平台 bundle 与 updates.json 到 `LAUNCHER_FILES_BUCKET`（如果配置）；支持 R2 checksum 特殊设置。
7. `gh release create v<version>` 发布 GitHub Release（nsis exe/dmg/AppImage/deb/rpm + notes）。

### CI（turbo-ci.yml）要点

- 步骤：checkout → Node/corepack → pnpm store 缓存 → apt 依赖 → rust toolchain(clippy,rustfmt) → cargo 缓存 → mold → binstall nextest → `pnpm install` → `cp .env.staging .env` → `pnpm run ci` → 验证 `intl:extract` + `intl:extract-packrinth` + `prune-local` 无 diff。
- 环境变量：`RUSTFLAGS=-Dwarnings`，`NEXTEST_NO_TESTS=pass`，`RUST_MIN_STACK=134217728`。
- 触发：仅 PR → main 与手工 dispatch（已移除 push main / merge_group 与 skip-if-clean 优化）。

### check-rust.yml

- `cargo binstall cargo-shear` → `cargo shear`（移除未用依赖）。
  - `SQLX_OFFLINE: true`。

### check-generic.yml

- `crate-ci/typos` 拼写扫描（排除见 `_typos.toml`）
- `tombi lint` + `tombi fmt --check`（TOML 防呆）

### 需要的 secrets/vars（完整清单）

| 名称 | 用途 |
| ------ | ------ |
| `TAURI_PRIVATE_KEY` / `TAURI_KEY_PASSWORD` | update 签名/验签（缺失 → 未签名构建） |
| `APPLE_CERTIFICATE` / `APPLE_CERTIFICATE_PASSWORD` / `APPLE_SIGNING_IDENTITY` / `APPLE_ID` / `APPLE_TEAM_ID` / `APPLE_PASSWORD` | macOS 签名与公证（任意缺失 → 未签名构建） |
| `DIGICERT_ONE_SIGNER_*`（API_KEY, CLIENT_CERTIFICATE_BASE64, CLIENT_CERTIFICATE_PASSWORD） | Windows 联署（缺失 → 未签名构建） |
| `LAUNCHER_FILES_BUCKET_*`（ACCESS_KEY_ID/SECRET_ACCESS_KEY 为 Secret；NAME/REGION/ENDPOINT_URL 为 Variable）+ Variable `LAUNCHER_FILES_BUCKET_BASE_URL` | 更新文件服务器（R2/S3）；详见 [repo-config.md](repo-config.md) |
| `CROWDIN_PROJECT_ID`（var）/ `CROWDIN_PERSONAL_TOKEN`（secret） | i18n 拉取（i18n-pull） |

## 更新 manifest 部署

`updates.json` 需能被 Tauri updater 插件访问（由 `tauri-plugin-updater` 的端点配置指向，最终 URL 为 `LAUNCHER_FILES_BUCKET_BASE_URL/updates.json`）。当前 `LAUNCHER_FILES_BUCKET_BASE_URL` 在 release workflow 中为空字符串，意味着尚未接入更新服务器——首次正式发布前需配置 bucket 并设置该值；未签名构建无 `.sig`，release 会自动跳过 manifest/S3 上传，仅发布 GitHub Release。

## 常见坑

- **本地打包 `PackageNotFound`/webview 报错**：确认 `pnpm install` 后同时有前端 dist；tauri build 前 frontend 会被 turbo 构建进资源（tauri.conf.json 的 `beforeBuildCommand` 检查）。
- **Linux 缺 `libayatana-appindicator3`**：rpm/deb 打包失败（Tauri tray 依赖）。
- **版本注入未生效**：CI 直接改 Cargo.toml；本地手动打包时确保 apps/app/Cargo.toml 与 app-lib 版本一致。
- **无 secrets 的手动 dispatch**：构建自动降级为未签名（环境变量为空时不联网无证书）；Windows 构建仅当 `tag-type=release` 或打开 sign 开关才尝试签名。
- **更新器 401/404**：检查 `LAUNCHER_FILES_BUCKET_BASE_URL` 是否配置，updates.json 路径与 bucket 首版是否已上传。
- **发布失败校验**：release workflow 要求 `v<version>` 是真实存在的 tag 且指向运行时 SHA；否则直接报错退出。
