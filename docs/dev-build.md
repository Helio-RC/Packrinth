# 构建与开发细节

本文档汇总 Packrinth 开发中高频接触的事实：pnpm/turbo 指令含义、构建方式（手动 vs GitHub Actions）、i18n 存储与同步、环境文件与 CI 工作流。适合新加入的开发者快速上手。

## 仓库结构速览

- `apps/app` — Packrinth 桌面壳（Tauri 2 + theseus，二进制 crate `theseus_gui`）
- `apps/app-frontend` — 前端 UI（Vue 3 + Vite，Tauri 内嵌页面）
- `apps/app-playground` — 测试用的独立 Rust 程序（`cargo run`/`cargo build`）
- `packages/` — app-lib（theseus 核心）、ui（共享组件）、api-client、assets、ariadne、daedalus、blog、utils 等
- 构建编排：Turborepo（`turbo.jsonc`）+ pnpm workspaces（`pnpm-workspace.yaml`，目录为 `apps/*` 和 `packages/*`）

## 环境要求

- Node.js ≥ 24.15.0（`.nvmrc`，CI 用 corepack 启用 pnpm 10.33.2）
- Rust 1.95.0（`rust-toolchain.toml`，workspace edition 2024）

## pnpm 指令（根目录）

| 指令 | 作用 |
| ------ | ------ |
| `pnpm app:dev` | 开发运行主应用：`turbo run dev --filter=@modrinth/app`，即 `tauri dev --features export-app-events`（前端热重载 + Rust 调试，调试期较长） |
| `pnpm app:build` | 构建主应用：`turbo run build --filter=@modrinth/app`，即 `tauri build`（产出安装包） |
| `pnpm build` | 全 workspace `build`（含 Rust/Python 各包） |
| `pnpm lint` | 全 workspace `lint` + `lint:ancillary`；ancillary 为 prettier 检查 `.github` 与根目录文件 |
| `pnpm lint:ancillary` | prettier `--check .github *.*` |
| `pnpm test` | 全 workspace `test`（Rust 走 `cargo nextest run --all-targets --no-fail-fast`；app-frontend 为 `vue-tsc --noEmit`） |
| `pnpm fix` | 自动修复：`fix`（eslint/clippy autofix + fmt）+ `fix:ancillary`（prettier `--write`） |
| `pnpm ci` | CI 入口：`turbo run lint test --continue` |
| `pnpm prepr` | 全部 workspace 的 `prepr`（fix + i18n extract/prune-local） |
| `pnpm prepr:frontend` | 仅 `@modrinth/app-frontend` 的 prepr |
| `pnpm prepr:frontend:app` | **开 PR 前必跑**：`prepr --filter=@modrinth/app-frontend`（同 prepr:frontend） |
| `pnpm prepr:frontend:lib` | prepr：ui/assets/blog/api-client/utils/tooling-config |
| `pnpm storybook` | 先 `i18n:coverage` 再启动 `@modrinth/ui` Storybook |
| `pnpm build-storybook` | 构建 `@modrinth/ui` Storybook 静态站 |
| `pnpm icons:add` | 向 `@modrinth/assets` 添加图标 |
| `pnpm i18n:coverage` | 检查/生成语言支持度文件 `language-settings-coverage.generated.ts` |
| `pnpm changelog:collect` | 合并各包 changelog 到统一数据（`collect-changelog`） |
| `pnpm changelog:combine-for-app` | 从 changelog 生成发布说明（`build-theseus-release-notes`，在发布流程中被调用） |
| `pnpm scripts <name>` | 运行 `scripts/<name>.ts` 任意脚本（如 `pnpm scripts i18n-icu-contract prune-local`） |

## 工作区子包指令

### apps/app（`@modrinth/app`）

| 指令 | 作用 |
| ------ | ------ |
| `pnpm --filter @modrinth/app tauri dev --features export-app-events` | 开发运行（app:dev 实际内容） |
| `pnpm --filter @modrinth/app build` | `tauri build`（生产打包路由见下） |
| `pnpm --filter @modrinth/app test` | `cargo nextest run --all-targets --no-fail-fast` |
| `pnpm --filter @modrinth/app lint` | `cargo fmt --check` + `cargo clippy --all-targets` + `--features updater` |

### apps/app-frontend（`@modrinth/app-frontend`）

| 指令 | 作用 |
| ------ | ------ |
| `pnpm --filter @modrinth/app-frontend dev` | `vite`（独立跑前端，不经 Tauri） |
| `pnpm --filter @modrinth/app-frontend build` | `vue-tsc --noEmit` + `vite build` |
| `pnpm --filter @modrinth/app-frontend lint` | `eslint .` + `prettier --check .` |
| `pnpm --filter @modrinth/app-frontend fix` | `eslint . --fix` + `prettier --write .` |
| `pnpm --filter @modrinth/app-frontend intl:extract` | formatjs 从源码提取字面量到 `src/locales/en-US/index.json` |
| `pnpm --filter @modrinth/app-frontend intl:prune-local` | 剔除当地语言中已被移除的 key（调用根脚本 i18n-icu-contract prune-local） |

### apps/app-playground

- `dev`：`cargo run`；`build`：`cargo build --release`；`test`：cargo nextest；`lint`：fmt + clippy

## 手动构建步骤（本地）

```bash
pnpm install                          # 安装依赖（pnpm 10，corepack）
cp packages/app-lib/.env.staging packages/app-lib/.env   # 首次：准备环境文件
pnpm app:dev                          # 开发运行
```

打包（生产）需要先设置环境文件并选择配置：

```bash
# 方式 A：直接 tauri build（使用 tauri.conf.json，适合快速验证）
pnpm app:build

# 方式 B：仿照 CI 流程（apps/app 提供 tauri-dev/release.conf.json 配置选择）
pnpm --filter @modrinth/app tauri build --config tauri-release.conf.json
```

补充说明：

- `apps/app/Cargo.toml` 的 version 与 `packages/app-lib/Cargo.toml`、`apps/app-frontend/package.json` 的 version 在 CI 发布时由 dasel 覆盖写入；本地开发用 `1.0.0-local`。
- 本地无签名时，Windows NSIS 构建应确认 `tauri-release.conf.json` 中 `bundle.windows.signCommand` 不干扰；CI 在非发布分支自动删除该字段。
- Linux 构建依赖（Debian/AppImage/rpm）需要 `libwebkit2gtk-4.1-dev`、`libayatana-appindicator3-dev`、`librsvg2-dev`、`libcurl4-openssl-dev`、cmake 等。
- app-frontend 的类型检查已在 `build`/`test` 内顺带执行（`vue-tsc --noEmit`）。

## GitHub Actions 构建流程

| Workflow | 触发 | 作用 |
| ------ | ------ | ------ |
| `theseus-build.yml`（Packrinth App build） | PR → `main`（路径过滤 app 闭合）或手工 dispatch（分支/tag、tag-type、release-tag-suffix、环境、Windows 签名开关） | 三平台矩阵构建（macos-latest / windows-latest / ubuntu-latest），产出 AppImage/deb/rpm、app.tar.gz/dmg、NSIS 安装包，上传 artifacts；release 类型自动触发 release workflow |
| `theseus-release.yml`（Packrinth App release） | 手工 dispatch（version + build-run-id；release 构建后由 build 自动触发） | 校验 tag 与 SHA、按 run-id 下载 artifacts、生成更新 manifest（updates.json）、上传至 S3（可选）、创建 GitHub Release |
| `turbo-ci.yml`（CI） | PR → main / 手工 dispatch | `pnpm ci`（全 workspace lint + test）；并验证 `intl:extract` + `intl:extract-packrinth` 已运行（git diff 检查 locales） |
| `check-rust.yml` | PR → main / 手工 dispatch | `cargo shear`（检查多余依赖） |
| `check-generic.yml` | PR → main / 手工 dispatch | `typos` 拼写检查 + `tombi lint/fmt`（TOML 校验） |
| `i18n-pull.yml` | 每周一 7:00 或手工 | 从 Crowdin 下载翻译、prune-local 清理失效 key、自动开 PR（crowdin-pull/<branch>）；**推送已有：已删除 i18n-push.yml** |
| `cancel-pr-workflow-on-merge.yml` | 合并 | 取消已合并 PR 的剩余 workflow |

### 升级/发布流程（手动 dispatch 驱动）

1. 在目标提交打 tag：`git tag v0.5.0 && git push origin v0.5.0`
2. 手动运行 **Packrinth App build**：`branch: v0.5.0`、`tag-type: release`、suffix 留空 → 使用 `tauri-release.conf.json`、`.env.prod`；有签名密钥则签名，无则自动降级未签名。
3. build 成功 → 自动触发 release：生成 changelog（`scripts/build-theseus-release-notes.ts`）、输出 `updates.json`（需各平台 `.sig`，未签名跳过）、上传 artifacts、创建 GitHub Release。
4. 若配置了 `LAUNCHER_FILES_BUCKET_*` secret 与 `LAUNCHER_FILES_BUCKET_BASE_URL`，更新服务上线；否则 updater 尚未接入（release workflow 里该值为空）。

### CI 注意点

- turbo-ci 在 `packages/app-lib` 下执行 `cp .env.staging .env` 后才跑 `pnpm ci`。
- CI 中 `RUSTFLAGS=-Dwarnings` 强制警告不通过；本地开发不启用。
- `SQLX_OFFLINE: true`：labrinth 相关离线 SQL，app 闭合不受影响。

## 环境文件（packages/app-lib/.env.*）

| 文件 | 用途 |
| ------ | ------ |
| `.env.local` | 本地默认（安装开发环境用） |
| `.env.staging` | 预发环境（开发、CI 默认使用；包含 API 端点配置） |
| `.env.prod` | 生产环境（发布构建使用） |
| `.env.prod-with-staging-archon` | 生产 API + staging archon 混合（archon 为 Modrinth 的管理后段？通常用于联合测试） |

变量以 `MODRINTH_URL`、`MODRINTH_API_BASE_URL`、`MODRINTH_ARCHON_BASE_URL`、`SHARED_INSTANCES_API_BASE_URL` 为主；这些变量也声明在 `turbo.jsonc` 的 `globalEnv` 中。

## i18n 文件存储位置

| 位置 | 内容 |
| ------ | ------ |
| `apps/app-frontend/src/locales/<locale>/index.json` | app 前端 UI 文案（en-US 为源文件，被 `intl:extract` 自动生成/更新） |
| `packages/ui/src/locales/<locale>/index.json` | 共享组件库 UI 文案 |
| `packages/ui/src/layouts/wrapped/settings/language-settings/language-settings-coverage.generated.ts` | 生成的各语言覆盖度清单（`pnpm i18n:coverage` 维护；覆盖是 UI 展示语言选择器的基础） |

- 源语言固定 `en-US`，当前共有 33 个语言文件夹（ar-SA 至 zh-TW）。
- 语言切换入口：`packages/ui/src/layouts/wrapped/settings/language-settings/`。

## i18n 同步方法与约定

### 提取新键（本地）

```bash
pnpm --filter @modrinth/app-frontend intl:extract   # app-frontend
pnpm --filter @modrinth/ui intl:extract             # ui（在 packages/ui 中运行）
```

- 提取后 `en-US/index.json` 会新增键；其他语言文件不变（由 Crowdin 翻译）。
- 要求保持 source 文件整洁：CI 会检查 `intl:extract` 是否产生 diff 并拒绝 PR。

### 清理失效键（本地）

```bash
pnpm scripts i18n-icu-contract prune-local
# 或按 scope：
pnpm --filter @modrinth/app-frontend intl:prune-local
```

### 拉取（Crowdin）

- 配置：`crowdin.yml`（源目录映射、`CROWDIN_PROJECT_ID`/`CROWDIN_PERSONAL_TOKEN`）。
- 下载：`i18n-pull.yml` 每周一 7:00 拉取并开 PR，或 workflow_dispatch 手动触发。
- 不再上传：`i18n-push.yml` 已删除（Crowdin 源文件由上游 Modrinth 维护；Packrinth 自有文案位于 `locales-packrinth/` 不被 Crowdin 管理）。

### 代码中的使用约定

- 组件内用 `@modrinth/ui` 的 `useVIntl()` / `formatMessage` / `IntlFormatted`。
- Packrinth 自有组件文案写入 `locales-packrinth/`（`intl:extract-packrinth`），切勿直接混入上游 `locales/`。
- 未翻译代码的 i18n 转换规范见 `.github/instructions/i18n-convert.instructions.md`。

## 工作流规范文档

CI 行为的具体规格说明位于 `spec/`（`spec-process-cicd-*.md`），修改 workflow 时请同步更新对应 spec。

## 常见陷阱

- **未运行 prepr 就开 PR**：CI 会失败（lint + intl:extract 校验）。开 PR 前跑 `pnpm prepr:frontend:app`。
- **NODE 版本**：需 ≥ 24.15；本地用 `.nvmrc` 管理。
- **Rust 目标**：macOS 构建需 `x86_64-apple-darwin` target（CI 已自动安装）；本机打 macOS 包需 `rustup target add x86_64-apple-darwin`。
- **不把 `.superpowers/`、`docs/superpowers/` 提交**：已 gitignore；这些是规划任务过程中的临时产物。
- **本地跑起 app 前**：先 `cp packages/app-lib/.env.staging packages/app-lib/.env`（若缺失，环境文件不存在会导致运行错误）。
