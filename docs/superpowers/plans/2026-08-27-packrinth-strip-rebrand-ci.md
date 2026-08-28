# Packrinth 仓库瘦身 + 产品改名 + CI 深化 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Modrinth 全平台 monorepo 瘦身为仅含 Packrinth 桌面应用（app + app-frontend + app-playground + app 依赖包）的仓库，产品改名 Packrinth，移除广告适配器，并把 CI 从 18 个工作流深化为 8 个带规格文档的工作流。

**Architecture:** 三个阶段，每个阶段结束均验证可编译。阶段 A 按已验证的依赖闭包删除非 app 模块并精简根清单（Cargo.toml / package.json / turbo.jsonc）；阶段 B 改名产品与移除广告（apps/app + app-frontend，不动 theseus 库）；阶段 C 删除死工作流、为保留的 8 个工作流写规格（spec/ 目录）、按规格重写 build/release/CI 三个核心工作流。

**Tech Stack:** Rust workspace (edition 2024) · pnpm + Turborepo · Tauri 2 · GitHub Actions · cargo-shear · actionlint

**Spec:**
- 设计基线：`apps/app/docs/goal.md`（§1.1 移除广告与版权内容、§2.1 五处入侵点、§9.4 CI/CD 要求）
- 依赖闭包分析：`/tmp/architecture-review-20260827-013826.html`（C1 仓库瘦身 / C2 CI 深化 / C3 发布契约 / C4 广告移除）
- 已确认决策：广告本次一并移除；app-playground 保留；app i18n 保留（删 web 条目）；产品本体改名 Packrinth

## Global Constraints

- 工作目录 `/projects/Packrinth`，当前分支 `develop`（勿切换分支）
- 环境：`export PATH="$HOME/.cargo/bin:$PATH"`
- Rust 验证命令（与仓库既有 task brief 一致）：`cargo check --manifest-path apps/app/Cargo.toml --no-default-features`（0 errors）+ `cargo test --manifest-path apps/app/Cargo.toml --no-default-features --bin theseus_gui`（全绿，当前 ≥119）
- 前端验证命令：`cd apps/app-frontend && npx vue-tsc --noEmit`（0 errors）`&& pnpm build`（通过）
- 缩进使用 TAB；不添加代码注释
- 提交前缀风格：`chore(repo):` / `ci:` / `feat(app):` / `docs(ci):`（仓库历史中英混用均可）
- **不修改 theseus 库源码**（packages/app-lib）——广告移除只动 `apps/app` + `apps/app-frontend`
- 保留 `main`/`develop` 分支策略与 `v*` tag 发布流程
- 保留 goal.md §2.1 的 5 处入侵点不变；广告移除为已记录在案的第 6 处差异
- 编辑 `.github/workflows/*.yml` 前必须加载 `authoring-github-workflows` skill，全部用 actionlint 验证（下载固定版本二进制，见该 skill）
- 规格文件必须使用 `create-github-action-workflow-specification` skill 的模板（`spec/spec-process-cicd-*.md`）
- 根 `Cargo.toml` 的 `[workspace.dependencies]` 清理以 cargo-shear 输出为准迭代进行，禁止凭记忆一次性删光

---

### Task 1: 删除非 app 的 apps、labrinth-only/孤儿 packages 与后端基础设施文件

**Files:**
- Delete: `apps/frontend/` `apps/labrinth/` `apps/docs/` `apps/daedalus_client/`（整个目录）
- Delete: `packages/moderation/` `packages/modrinth-log/` `packages/modrinth-maxmind/` `packages/modrinth-util/` `packages/muralpay/` `packages/neverbounce/` `packages/sqlx-tracing/` `packages/xredis/` `packages/component-derive/`（整个目录）
- Delete: `docker-compose.yml` `scripts/clone-labrinth-projects.mjs` `scripts/import-projects.py`
- Delete: `.github/ISSUE_TEMPLATE/2-web-bug.yml` `3-hosting-bug.yml` `4-api-bug.yml`
- Delete: `.github/assets/api_cover.png` `web_cover.png` `monorepo_cover.png`（保留 `app_cover.png`）

**Interfaces:**
- Produces: 已删除但清单仍引用这些成员的"broken tree"状态 —— 本任务**不提交**，Task 4 统一提交

- [ ] **Step 1: 确认基线**

```bash
cd /projects/Packrinth && git status --porcelain && git branch --show-current
```
Expected: 工作树干净，分支 `develop`

- [ ] **Step 2: 删除 apps**

```bash
git rm -r apps/frontend apps/labrinth apps/docs apps/daedalus_client
```

- [ ] **Step 3: 删除 packages**

```bash
git rm -r packages/moderation packages/modrinth-log packages/modrinth-maxmind packages/modrinth-util packages/muralpay packages/neverbounce packages/sqlx-tracing packages/xredis packages/component-derive
```

- [ ] **Step 4: 删除后端基础设施与模板文件**

```bash
git rm docker-compose.yml scripts/clone-labrinth-projects.mjs scripts/import-projects.py .github/ISSUE_TEMPLATE/2-web-bug.yml .github/ISSUE_TEMPLATE/3-hosting-bug.yml .github/ISSUE_TEMPLATE/4-api-bug.yml .github/assets/api_cover.png .github/assets/web_cover.png .github/assets/monorepo_cover.png
```

- [ ] **Step 5: 核对删除结果**

```bash
ls apps/ packages/ scripts/
```
Expected: `apps/` 仅剩 `app` `app-frontend` `app-playground`；`packages/` 仅剩 app 闭包 13 个（app-lib, api-client, ariadne, assets, async-minecraft-ping, blog, daedalus, modrinth-content-management, path-util, serde-binhum, tooling-config, ui, utils）

**注意：本任务不提交、不验证编译 —— 清单尚未精简，树是有意 broken 的。继续 Task 2。**

---

### Task 2: 精简根 Cargo.toml（members / workspace.dependencies / profile）

**Files:**
- Modify: `Cargo.toml`（根）
- Modify: `apps/app/Cargo.toml` `packages/app-lib/Cargo.toml` `packages/daedalus/Cargo.toml`（`repository` 字段）

**Interfaces:**
- Consumes: Task 1 的删除结果
- Produces: 只含 app 闭包成员的 Rust 工作区；后续 Task 4 验证 `cargo check --workspace`

- [ ] **Step 1: 精简 members**

编辑根 `Cargo.toml` `[workspace] members`，移除 `"apps/daedalus_client"` 与 `"apps/labrinth"`。保留其余 13 项（含 apps/app、apps/app-playground、packages/app-lib、packages/ariadne、packages/component-derive、packages/daedalus、packages/modrinth-content-management、packages/path-util、packages/serde-binhum、packages/modrinth-log、packages/modrinth-maxmind、packages/modrinth-util、packages/neverbounce、packages/xredis）——注意：其中 modrinth-log/maxmind/util、neverbounce、xredis、component-derive 目录已在 Task 1 删除，**一并从 members 移除**，members 最终只保留：`apps/app`、`apps/app-playground`、`packages/app-lib`、`packages/ariadne`、`packages/daedalus`、`packages/modrinth-content-management`、`packages/path-util`、`packages/serde-binhum`

- [ ] **Step 2: 更新 workspace 元信息**

根 `Cargo.toml` `[workspace.package] repository` 改为 `https://github.com/Helio-RC/Packrinth`；同步改 `apps/app/Cargo.toml`、`packages/app-lib/Cargo.toml`、`packages/daedalus/Cargo.toml` 的 `repository` 字段

- [ ] **Step 3: 删除已知 labrinth-only 的 workspace.dependencies**

删除以下条目（高置信 labrinth/daedalus/web 专用，目录已删）：`actix-cors` `actix-files` `actix-http` `actix-multipart` `actix-rt` `actix-web` `actix-web-prom` `actix-ws` `argon2` `async-stripe` `aws-sdk-s3` `clickhouse` `censor` `chumsky` `deadpool-redis` `dotenv-build` `dotenvy` `jemalloc_pprof` `lettre` `maxminddb` `modrinth-log` `modrinth-maxmind` `modrinth-util` `murmur2` `neverbounce` `prometheus` `rdkafka` `redis` `rust_decimal` `rust_iso3166` `rust-s3` `rusty-money` `scalar_api_reference` `secrecy` `sentry` `sqlx-tracing` `tikv-jemalloc-ctl` `tikv-jemallocator` `totp-rs` `tracing-actix-web` `tracing-ecs` `utoipa` `validator` `webauthn-rs` `webauthn-rs-proto` `woothee` `xredis` `zxcvbn` `yaserde` `muralpay`

保留（app 闭包在用，勿删）：`async-minecraft-ping` `async-trait` `async-tungstenite` `async-walkdir` `async_zip` `base64` `bitflags` `bon` `bytemuck` `bytes` `chardetng` `chrono` `cidre` `clap` `color-eyre` `color-thief` `component-derive`（若确认已无引用则随 cargo-shear 删除）`const_format` `core-foundation` `core-graphics` `daedalus` `darling` `dashmap` `data-url` `derive_more` `directories` `dirs` `discord-rich-presence` `dunce` `either` `encoding_rs` `enumset` `eyre` `flate2` `fs4` `futures` `futures-lite` `futures-util` `governor` `heck` `hex` `hickory-resolver` `hmac` `httpdate`（若 shear 报未用再删）`hyper`（若报未用再删）`iana-time-zone` `image` `indexmap` `indicatif` `itertools` `json-patch` `json5` `lz4_flex` `native-dialog` `notify` `notify-debouncer-mini` `objc2-app-kit` `p256` `parking_lot` `paste` `path-util` `phf` `png` `postcard` `postcard-bindgen` `proc-macro2` `quartz_nbt` `quick-xml` `quote` `rand` `rand_chacha` `regex` `reqwest` `rgb` `rustls` `serde` `serde-binhum` `serde_bytes` `serde_cbor` `serde_ini` `serde_json` `serde_with` `sha1` `sha2` `shlex` `smallvec` `spdx` `sqlx`（theseus 用 sqlx/sqlite）`strum` `syn` `sysinfo` `tar` `tauri` `tauri-build` `tauri-plugin-deep-link` `tauri-plugin-dialog` `tauri-plugin-fs` `tauri-plugin-http` `tauri-plugin-opener` `tauri-plugin-os` `tauri-plugin-single-instance` `tauri-plugin-updater`（git 依赖）`tauri-plugin-window-state` `tempfile` `theseus` `thiserror` `tokio` `tokio-stream` `tokio-util` `toml` `tracing` `tracing-error` `tracing-subscriber` `ts-rs` `typed-path` `url` `urlencoding` `uuid` `webp` `webview2-com` `whoami` `windows` `windows-core` `winreg` `zip`

- [ ] **Step 4: 删除 release-labrinth profile**

删除根 `Cargo.toml` 末尾 `[profile.release-labrinth]` 整段

- [ ] **Step 5: cargo-shear 迭代清理残留**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo shear
```
Expected: shear 报出仍未使用的 workspace 依赖（如 `httpdate` `hyper` `component-derive` 等）。逐个删除其 `[workspace.dependencies]` 条目后重跑，直到 `cargo shear` 输出无未使用依赖警告

- [ ] **Step 6: 编译验证**

```bash
cargo check --workspace
```
Expected: 0 errors。若有报缺失依赖的 crate，恢复对应 workspace 条目

**注意：本任务不提交。继续 Task 3。**

---

### Task 3: 精简根 package.json / turbo.jsonc / crowdin.yml / .mergify.yml / README / AGENTS.md / i18n 脚本

**Files:**
- Modify: `package.json`（根）`turbo.jsonc` `crowdin.yml` `.mergify.yml` `README.md` `AGENTS.md` `scripts/coverage-i18n.ts` `scripts/i18n-import-check.ts`

**Interfaces:**
- Consumes: Task 1-2 的删除与精简
- Produces: 根 JS 工具链只面向 app 闭包；Task 4 统一验证与提交

- [ ] **Step 1: 精简根 package.json scripts**

删除：`web:dev` `web:build` `pages:build` `docs:dev` `prepr:frontend:web`
修改：`prepr:frontend:lib` 的 filter 列表去掉 `@modrinth/moderation`（保留 ui/assets/blog/api-client/utils/tooling-config）
保留：`app:dev` `app:build` `build` `lint` `lint:ancillary` `test` `fix` `fix:ancillary` `ci` `prepr` `prepr:frontend` `prepr:frontend:app` `storybook` `build-storybook` `icons:add` `i18n:coverage` `changelog:collect` `changelog:combine-for-app` `scripts`
devDependencies 全部保留（如 `@crowdin/crowdin-api-client` 被 i18n 脚本使用）

- [ ] **Step 2: 精简 turbo.jsonc**

- `//#i18n:coverage` 的 `inputs`：删除 `"apps/frontend/src/**/*.vue"` `"apps/frontend/src/locales/**/*.json"` `"packages/moderation/src/**/*.vue"` `"packages/moderation/src/locales/**/*.json"` 四项
- `globalEnv`：删除 `VITE_STRIPE_PUBLISHABLE_KEY`（web 专用）；保留 `MODRINTH_URL` `MODRINTH_API_BASE_URL` `SHARED_INSTANCES_API_BASE_URL` `MODRINTH_ARCHON_BASE_URL`（theseus 运行时使用）
- `test` 任务的 `env`：删除 `REDIS_*` 与 `DATABASE_URL`；保留 `SQLX_OFFLINE`（theseus 的 sqlx 离线模式）与 `CARGO_*` `RUST_*` `RUSTFLAGS` `FORCE_COLOR` `NEXTEST_*`
- `build` 任务的 `env`：删除 `REDIS_*` 相关（若有）；保留 `SQLX_OFFLINE` `DATABASE_URL`（theseus sqlx 需要）

- [ ] **Step 3: 精简 crowdin.yml**

删除 `apps/frontend` 与 `packages/moderation` 两个 `files:` 条目，仅保留 `apps/app-frontend` 与 `packages/ui` 条目

- [ ] **Step 4: 精简 .mergify.yml**

删除 `frontend` 与 `backend` 两个 scope；`app` scope 中删除不存在的 `packages/app-macros/**` 行

- [ ] **Step 5: 精简 scripts**

- `scripts/coverage-i18n.ts`：删除其中 `apps/frontend/src` 与 `packages/moderation/src` 的扫描路径（保留 app-frontend 与 ui）
- `scripts/i18n-import-check.ts`：删除 `apps/frontend/src` 扫描路径
- 确认 `scripts/run.mjs` 注册表不再引用已删除脚本（`clone-labrinth-projects`、`import-projects` 已在 Task 1 删除；若 run.mjs 中仍列出则一并删除注册项）

- [ ] **Step 6: 重写 README.md**

简短重写：项目名 Packrinth（基于 Modrinth App 二次开发的 AI 模组包制作器）、技术栈（Tauri 2 + theseus + Vue 3）、开发命令（`pnpm app:dev`）、分支策略（upstream/main → main → develop）、指向 `apps/app/docs/goal.md`

- [ ] **Step 7: 更新根 AGENTS.md**

- apps 表格删除 `frontend` `labrinth` `docs` `daedalus_client` 行，`app` 描述改为 Packrinth
- packages 表格删除已删包，`app-lib` 描述改为 theseus 核心库
- 删除对 `apps/labrinth/AGENTS.md` 与 `apps/frontend/AGENTS.md` 的引用
- Pre-PR 命令只保留 app 相关（`pnpm prepr:frontend:app`）

- [ ] **Step 8: 重新安装依赖并验证 JS 层**

```bash
cd /projects/Packrinth && pnpm install
pnpm i18n:coverage
```
Expected: install 成功（lockfile 已去除已删包）、coverage 生成无报错

**注意：本任务不提交。继续 Task 4。**

---

### Task 4: 仓库级验证 + 提交（阶段 A 完成）

**Files:**
- 无新增；验证 Task 1-3 的结果

- [ ] **Step 1: 全量 Rust 验证**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo check --workspace
cargo check --manifest-path apps/app/Cargo.toml --no-default-features
```
Expected: 均 0 errors

- [ ] **Step 2: 全量前端验证**

```bash
cd /projects/Packrinth/apps/app-frontend && npx vue-tsc --noEmit && pnpm build
```
Expected: 0 errors，build 通过

- [ ] **Step 3: 检查残留引用**

```bash
cd /projects/Packrinth && grep -rn "apps/frontend\|apps/labrinth\|apps/docs\|apps/daedalus\|packages/moderation\|modrinth-util\|muralpay\|neverbounce\|xredis\|sqlx-tracing\|component-derive" --include="*.json" --include="*.ts" --include="*.mjs" --include="*.js" --include="*.toml" --include="*.yml" --include="*.yaml" --include="*.md" -l | grep -v node_modules | grep -v pnpm-lock
```
Expected: 无输出（或仅剩 apps/app/docs/goal.md 等历史文档提及，属正常，逐条确认）

- [ ] **Step 4: 提交**

```bash
git add -A && git commit -m "chore(repo): strip monorepo to Packrinth app core"
```
（该提交包含 Task 1-3 的全部删除与精简）

---

### Task 5: 产品改名 Packrinth（应用本体）

**Files:**
- Modify: `apps/app/tauri.conf.json` `apps/app/tauri.linux.conf.json` `apps/app/tauri.macos.conf.json` `apps/app/tauri-release.conf.json` `apps/app/Info.plist` `apps/app/App.entitlements`（若含名称）
- Check: `apps/app/Cargo.toml`（crate 名 `theseus_gui` **不改**）

**Interfaces:**
- Consumes: Task 4 的干净基线
- Produces: 产品级名称统一为 `Packrinth`；CI 工作流（Task 9-10）引用同一名称

- [ ] **Step 1: 修改 tauri 主配置**

`apps/app/tauri.conf.json`：
- `productName`: `"Packrinth"`
- `mainBinaryName`: `"Packrinth"`
- `identifier`: 改为 `"com.packrinth.app"`（当前值先读文件确认，通常为 `com.modrinth.theseus`）
- 其他字段（version、window 标题如含 "Modrinth" 一并改为 "Packrinth"）

- [ ] **Step 2: 修改平台配置**

- `tauri.linux.conf.json`：`productName`/`mainBinaryName` 覆盖值改为 `Packrinth`（原为 `ModrinthApp`）
- `tauri.macos.conf.json`：检查并同步
- `tauri-release.conf.json`：检查 `plugins.updater.endpoints` —— **执行时向用户询问**："Packrinth 的更新服务器基础 URL 是什么？（对应 CI release 工作流的 LAUNCHER_FILES_BUCKET_BASE_URL，例如你的 S3/R2 桶域名）"。用户给出后写入；若暂无更新服务器，将 `endpoints` 置空数组并在报告说明 updater 未配置

- [ ] **Step 3: 修改 macOS 元数据**

`Info.plist`：`CFBundleName` / `CFBundleDisplayName` / `CFBundleIdentifier` 同步为 Packrinth / com.packrinth.app（逐项读文件确认现值）；`App.entitlements` 若含 bundle 名一并改

- [ ] **Step 4: 检查应用内品牌字符串（仅配置文件层面）**

```bash
grep -rn "Modrinth App\|ModrinthApp" apps/app/tauri*.conf.json apps/app/Info.plist apps/app/App.entitlements apps/app/dmg/ 2>/dev/null
```
Expected: 无残留（UI 内文案字符串不在此任务范围，留待专门改名任务）

- [ ] **Step 5: 验证**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo check --manifest-path apps/app/Cargo.toml --no-default-features
jq . apps/app/tauri.conf.json > /dev/null && jq . apps/app/tauri-release.conf.json > /dev/null
```
Expected: 0 errors，JSON 均合法

- [ ] **Step 6: 提交**

```bash
git add -A && git commit -m "chore(app): rebrand product name to Packrinth"
```

---

### Task 6: 移除广告适配器（goal.md §1.1）

**Files:**
- Delete: `apps/app/src/api/ads.rs` `apps/app/src/api/ads-consent/`（整个目录）`apps/app/src/api/ads_occlusion_macos.rs` `apps/app/src/api/ads_occlusion_windows.rs`
- Delete: `apps/app-frontend/src/helpers/ads.js`
- Modify: `apps/app/src/api/mod.rs`（删 `pub mod ads;` 及 ads_occlusion 声明）`apps/app/src/main.rs`（删 `.plugin(api::ads::init())`，约 266 行）
- Modify: `apps/app-frontend/src/App.vue` `apps/app-frontend/src/pages/project/Gallery.vue` `apps/app-frontend/src/components/ui/SurveyPopup.vue` `apps/app-frontend/src/components/ui/PromotionWrapper.vue`（删除 ads.js 的 import 与 take/release_ads_window_hold 调用；PromotionWrapper 若仅为广告容器则整体删除，若还有其他用途保留骨架）
- Check（grep 确认后处理）: `apps/app-frontend/src/components/ui/settings/account/PrivacySettings.vue`（广告同意项 UI）

**Interfaces:**
- Consumes: Task 5 后的基线
- Produces: 应用不含广告代码；删除测试通过（无调用者）

- [ ] **Step 1: 确认前端广告引用面**

```bash
grep -rn "helpers/ads\|ads_window_hold\|ads-consent\|init_ads" apps/app-frontend/src --include="*.vue" --include="*.ts" --include="*.js" | grep -v locales
```
Expected: 列出 App.vue / Gallery.vue / SurveyPopup.vue / PromotionWrapper.vue 的引用点（与上述清单一致）

- [ ] **Step 2: 删除 Rust 广告模块**

```bash
git rm apps/app/src/api/ads.rs apps/app/src/api/ads-consent apps/app/src/api/ads_occlusion_macos.rs apps/app/src/api/ads_occlusion_windows.rs
```
编辑 `apps/app/src/api/mod.rs`：删除 `pub mod ads;` 与两个 ads_occlusion 的 mod 声明
编辑 `apps/app/src/main.rs`：删除 `.plugin(api::ads::init())` 一行

- [ ] **Step 3: 删除前端广告接线**

```bash
git rm apps/app-frontend/src/helpers/ads.js
```
- `App.vue`：删除 ads.js 相关 import 与 `take_ads_window_hold`/`release_ads_window_hold` 调用及事件监听
- `Gallery.vue`、`SurveyPopup.vue`：删除 import 与 hold/release 调用
- `PromotionWrapper.vue`：检查引用后删除（`grep -rn "PromotionWrapper" apps/app-frontend/src --include="*.vue"` 确认无其他引用）
- `PrivacySettings.vue`：删除广告相关设置项（若为广告同意开关）

- [ ] **Step 4: 检查 settings 联动**

```bash
grep -rn "ads" apps/app/src/settings* apps/app/src/api/settings.rs 2>/dev/null | head
```
若 theseus 设置含广告同意字段：**不改 theseus**（约束），仅确保前端不再读取展示

- [ ] **Step 5: 验证**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo check --manifest-path apps/app/Cargo.toml --no-default-features
cargo test --manifest-path apps/app/Cargo.toml --no-default-features --bin theseus_gui
cd apps/app-frontend && npx vue-tsc --noEmit && pnpm build
```
Expected: 0 errors，测试全绿（≥119），前端构建通过

- [ ] **Step 6: 提交**

```bash
git add -A && git commit -m "feat(app): remove Modrinth ad adapter per goal.md 1.1"
```

---

### Task 7: 删除死工作流（阶段 C 前置）

**Files:**
- Delete: `.github/workflows/api-client-release.yml` `changelog-comment.yml` `cmd-deploy.yml` `daedalus-docker.yml` `daedalus-run.yml` `frontend-deploy.yml` `frontend-docker.yml` `frontend-preview.yml` `labrinth-build.yml` `slash-cmds.yml`
- Keep: `cancel-pr-workflow-on-merge.yml` `check-generic.yml` `check-rust.yml` `i18n-pull.yml` `i18n-push.yml` `theseus-build.yml` `theseus-release.yml` `turbo-ci.yml`（共 8 个）

**Interfaces:**
- Produces: workflows 目录仅剩 8 个 app 相关文件；Task 8 为其写规格

- [ ] **Step 1: 删除**

```bash
cd /projects/Packrinth && git rm .github/workflows/api-client-release.yml .github/workflows/changelog-comment.yml .github/workflows/cmd-deploy.yml .github/workflows/daedalus-docker.yml .github/workflows/daedalus-run.yml .github/workflows/frontend-deploy.yml .github/workflows/frontend-docker.yml .github/workflows/frontend-preview.yml .github/workflows/labrinth-build.yml .github/workflows/slash-cmds.yml
```

- [ ] **Step 2: 核对**

```bash
ls .github/workflows/
```
Expected: 恰好 8 个保留文件

- [ ] **Step 3: 提交**

```bash
git add -A && git commit -m "ci: remove workflows for deleted modules"
```

---

### Task 8: 编写 8 个工作流规格（create-github-action-workflow-specification）

**Files:**
- Create: `spec/spec-process-cicd-app-build.md`（对应 theseus-build.yml）
- Create: `spec/spec-process-cicd-app-release.md`（对应 theseus-release.yml）
- Create: `spec/spec-process-cicd-ci.md`（对应 turbo-ci.yml）
- Create: `spec/spec-process-cicd-check-rust.md`
- Create: `spec/spec-process-cicd-check-generic.md`
- Create: `spec/spec-process-cicd-i18n-pull.md`
- Create: `spec/spec-process-cicd-i18n-push.md`
- Create: `spec/spec-process-cicd-pr-housekeeping.md`（对应 cancel-pr-workflow-on-merge.yml）

**Interfaces:**
- Consumes: 8 个保留工作流的当前内容
- Produces: 每个规格含触发条件、job 依赖、输入/输出契约（secrets、env、artifacts）、质量门禁、错误处理、验收标准 —— 是 Task 9-11 重写的依据与验证面

- [ ] **Step 1: 加载规格技能**

加载 `create-github-action-workflow-specification` skill，按其中模板与"Analysis Instructions"逐工作流分析

- [ ] **Step 2: 写 app-build 规格**

分析 `theseus-build.yml` 全量内容（矩阵平台、dasel 版本注入、签名 secrets、产物 globs），产出 `spec/spec-process-cicd-app-build.md`。**先按现状写（Modrinth App 名称）**，Task 9 重写后回到本文件更新为 Packrinth 契约

- [ ] **Step 3: 写 app-release 规格**

分析 `theseus-release.yml`（workflow_run 链、updates.json 结构、S3 上传、gh release），产出 `spec/spec-process-cicd-app-release.md`，重点记录 build→release 的**隐式契约**：产物名（`Modrinth App_${VERSION}_amd64.AppImage` 等 9 处 glob）、签名文件位置、`v*` tag 约定、6 个 S3 环境变量

- [ ] **Step 4: 写其余 6 个规格**

依次分析 `turbo-ci.yml`、`check-rust.yml`、`check-generic.yml`、`i18n-pull.yml`、`i18n-push.yml`、`cancel-pr-workflow-on-merge.yml`，产出对应规格文件。turbo-ci 规格需记录 labrinth 相关步骤（Redis 集群、sqlx setup）为"现状"，供 Task 11 对照删除

- [ ] **Step 5: 提交**

```bash
git add spec/ && git commit -m "docs(ci): add workflow specifications"
```

---

### Task 9: 重写 theseus-build.yml（Packrinth + PRODUCT_NAME 契约 + 标准 runners）

**Files:**
- Modify: `.github/workflows/theseus-build.yml`

**Interfaces:**
- Consumes: Task 5 的 Packrinth 产品名、Task 8 的 app-build 规格
- Produces: `PRODUCT_NAME=Packrinth` 单一来源；产物 glob 全部由它派生；这些us-release（Task 10）读取同一契约

- [ ] **Step 1: 加载工作流技能**

加载 `authoring-github-workflows` skill；按需下载 actionlint 固定版本二进制

- [ ] **Step 2: 名称与触发**

- `name:` 改为 `Packrinth App build`
- `on.push.paths`：删除不存在的 `'packages/app-macros/**'`；其余（apps/app、apps/app-frontend、packages/app-lib、packages/assets、packages/ui、packages/utils、本文件）保留
- 保留 `workflow_dispatch` 输入（sign-windows-binaries、environment、app-version-override）

- [ ] **Step 3: 环境与契约**

- 文件顶部 `env:` 增加 `PRODUCT_NAME: Packrinth`
- 删除 `VITE_STRIPE_PUBLISHABLE_KEY`（web 遗留）
- `RUSTC_WRAPPER: 'sccache'` 保留

- [ ] **Step 4: runners 与缓存**

矩阵平台替换（Modrinth 私有 namespace runners 不可用）：
- `namespace-profile-medium-amd64-macos` → `macos-latest`
- `namespace-profile-medium-amd64-windows` → `windows-latest`
- `namespace-profile-medium-amd64` → `ubuntu-latest`
删除 `namespacelabs/nscloud-cache-action` 步骤与 `nsc cache sccache setup` 步骤，替换为标准缓存：
- Rust：`actions/cache`（key: `cargo-${{ runner.os }}-${{ hashFiles('Cargo.lock') }}`，restore-keys: `cargo-${{ runner.os }}-`，path: `~/.cargo/registry`、`~/.cargo/git`、`target`）
- pnpm：`actions/setup-node` 后加 `pnpm config set store-dir` 指向缓存路径 + `actions/cache`（path: pnpm store，key: `pnpm-${{ runner.os }}-${{ hashFiles('pnpm-lock.yaml') }}`）
- sccache：保留 `mozilla-actions/sccache-action`（它自带缓存，不需要 nscloud）

- [ ] **Step 5: tauri-dev.conf.json 生成块**

生成内容改为：
```json
{
  "productName": "Packrinth (dev-${GIT_HASH})",
  "mainBinaryName": "Packrinth (dev-${GIT_HASH})",
  "identifier": "PackrinthApp-dev-${GIT_HASH}",
  "bundle": {
    "fileAssociations": []
  }
}
```

- [ ] **Step 6: 产物 globs**

Upload artifacts 的 path 全部按 `${{ env.PRODUCT_NAME }}` 派生（YAML 中直接写 `Packrinth` 亦可，但须与 release 工作流一致）：
- `target/release/bundle/appimage/Packrinth_*.AppImage*`、`target/release/bundle/deb/Packrinth_*.deb*`、`target/release/bundle/rpm/Packrinth-*.rpm*`
- dev 变体 `Packrinth (dev-*)_*` 同理
- macOS/Windows 同理（`Packrinth.app.tar.gz*`、`Packrinth_*.dmg`、`Packrinth_*-setup.exe*`、`Packrinth_*-setup.nsis.zip*`）
注意 glob 须同时覆盖 `Packrinth`（tag 构建）与 `Packrinth (dev-*)`（PR/手动构建）两种前缀

- [ ] **Step 7: 验证**

```bash
actionlint .github/workflows/theseus-build.yml
```
Expected: 无 error。再用 `bash -n` 无法直接校验（YAML 内联），以 actionlint 为准

- [ ] **Step 8: 提交**

```bash
git add .github/workflows/theseus-build.yml && git commit -m "ci: rebrand app build workflow for Packrinth"
```

---

### Task 10: 重写 theseus-release.yml

**Files:**
- Modify: `.github/workflows/theseus-release.yml`

**Interfaces:**
- Consumes: Task 9 的 `PRODUCT_NAME=Packrinth` 契约与产物命名
- Produces: 与 build 产物 glob 精确匹配的 release 流程；规格文件同步更新

- [ ] **Step 1: 名称与环境**

- `name:` 改为 `Packrinth App release`
- `env:` 增加 `PRODUCT_NAME: Packrinth`
- 保留 `LAUNCHER_FILES_BUCKET_BASE_URL` 与三个 artifact-name 环境变量

- [ ] **Step 2: 签名与 glob 路径**

- `macOsAarch64UpdateArtifactSignature` / `macOsX64UpdateArtifactSignature`：路径中 `Modrinth App.app.tar.gz.sig` → `Packrinth.app.tar.gz.sig`
- `linuxX64UpdateArtifactSignature`：`Modrinth App_${VERSION_TAG#v}_amd64.AppImage.tar.gz.sig` → `Packrinth_${VERSION_TAG#v}_amd64.AppImage.tar.gz.sig`
- `windowsX64UpdateArtifactSignature`：`Modrinth App_${VERSION_TAG#v}_x64-setup.nsis.zip.sig` → `Packrinth_${VERSION_TAG#v}_x64-setup.nsis.zip.sig`
- updates.json 的 `install_urls` 中 `Modrinth App_` 前缀 → `Packrinth_`（macos 的 `"Modrinth App.app.tar.gz"` 与 `"Modrinth App_" + $versionTag + "_universal.dmg"` 同理）

- [ ] **Step 3: GitHub release 命令**

```bash
gh release create "$VERSION_TAG" \
  --title "Packrinth ${VERSION}" \
  --notes-file release-notes.md \
  "${WINDOWS_X64_BUNDLE_ARTIFACT_NAME}/release/bundle/nsis/Packrinth_${VERSION}_x64-setup.exe" \
  "${MACOS_UNIVERSAL_BUNDLE_ARTIFACT_NAME}/universal-apple-darwin/release/bundle/dmg/Packrinth_${VERSION}_universal.dmg" \
  "${LINUX_X64_BUNDLE_ARTIFACT_NAME}/release/bundle/appimage/Packrinth_${VERSION}_amd64.AppImage" \
  "${LINUX_X64_BUNDLE_ARTIFACT_NAME}/release/bundle/deb/Packrinth_${VERSION}_amd64.deb" \
  "${LINUX_X64_BUNDLE_ARTIFACT_NAME}/release/bundle/rpm/Packrinth-${VERSION}-1.x86_64.rpm"
```

- [ ] **Step 4: 验证 + 同步规格**

```bash
actionlint .github/workflows/theseus-release.yml
```
Expected: 无 error。回到 `spec/spec-process-cicd-app-release.md` 更新"隐式契约"一节为 Packrinth 命名，并提交该更新

- [ ] **Step 5: 提交**

```bash
git add .github/workflows/theseus-release.yml spec/spec-process-cicd-app-release.md && git commit -m "ci: rebrand app release workflow for Packrinth"
```

---

### Task 11: 重写 turbo-ci.yml（去掉 labrinth 测试装备，只跑 app 闭包）

**Files:**
- Modify: `.github/workflows/turbo-ci.yml`

**Interfaces:**
- Consumes: Task 8 的 turbo-ci 规格（对照删除清单）
- Produces: CI 只需 Node + Rust 工具链即可跑通，无 Redis/docker/sqlx 前置

- [ ] **Step 1: runner 与缓存**

- `runs-on`：`namespace-profile-modrinth-turbo` → `ubuntu-latest`（`skip-if-clean` 输出分支逻辑可简化，但保留结构与 merge-queue-ci-skipper 兼容即可）
- 删除 `namespacelabs/nscloud-cache-action` 与 `namespace-actions/setup-turbocache` 步骤（含其 `if: needs.skip-if-clean.outputs.internal == 'true'` 条件块），替换为标准缓存（同 Task 9 Step 4：cargo registry + pnpm store，用 `actions/cache`）

- [ ] **Step 2: 删除 labrinth 装备**

删除以下步骤与 env：
- `env`：`REDIS_TOPOLOGY` `REDIS_CONNECTION_TYPE` `REDIS_URL`
- 步骤 `Setup cargo-sqlx`（taiki-e/cache-cargo-install-action）
- 步骤 `Check if labrinth tests need to run`（check-labrinth）
- 步骤 `Start services`（docker compose cluster-redis）
- 步骤 `Setup labrinth environment and database`（sqlx database setup）
保留：`NEXTEST_NO_TESTS: pass`、`RUSTFLAGS: -Dwarnings`、`RUST_MIN_STACK`、apt 依赖安装（webkit2gtk 等为 tauri/theseus 编译所需）、mold、nextest、intl:extract 校验步骤

- [ ] **Step 3: 验证**

```bash
actionlint .github/workflows/turbo-ci.yml
```
Expected: 无 error。确认工作流中不再出现 `docker`、`redis`、`sqlx`、`labrinth` 字样（`grep -i "redis\|labrinth\|docker\|sqlx" .github/workflows/turbo-ci.yml` 无输出）

- [ ] **Step 4: 提交**

```bash
git add .github/workflows/turbo-ci.yml && git commit -m "ci: scope CI to app closure"
```

---

### Task 12: 最终全量验证

**Files:**
- 无新增；收尾检查

- [ ] **Step 1: Rust 全量**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo check --workspace
cargo test --manifest-path apps/app/Cargo.toml --no-default-features --bin theseus_gui
cargo shear
```
Expected: 0 errors、测试全绿（≥119）、shear 无未用依赖

- [ ] **Step 2: 前端全量 + lint**

```bash
cd /projects/Packrinth/apps/app-frontend && npx vue-tsc --noEmit && pnpm build
cd /projects/Packrinth && pnpm prepr:frontend:app
```
Expected: 0 errors、build 通过、prepr（lint/fix/intl）通过

- [ ] **Step 3: 工作流全量校验**

```bash
for f in .github/workflows/*.yml; do actionlint "$f" || echo "FAIL: $f"; done
```
Expected: 全部无 error

- [ ] **Step 4: 结构终检**

```bash
ls apps/ packages/ .github/workflows/ spec/
git status --porcelain
```
Expected: 3 个 app / 13 个包 / 8 个工作流 / spec 8 个规格文件；工作树干净

- [ ] **Step 5: 提交遗留**

若 Step 4 有未提交变更：`git add -A && git commit -m "chore: final verification fixes"`（无变更则跳过）

---

## Self-Review 记录

- **规格覆盖**：goal.md §9.4（CI 跑 Mock+单元测试、覆盖率门禁、cargo audit）——当前计划的 CI 深化保留 lint/test/intl 检查；覆盖率门禁与 cargo audit 属新增能力，未纳入本次（工作流仅做删减与改名，不扩功能），已在 Top 建议中说明可后续以规格为准扩展。
- **占位符**：唯一执行期询问是 Task 5 Step 2 的 updater 端点 URL（用户私有基础设施，无法预填），已给出明确询问话术与未提供时的降级行为。
- **类型一致性**：Task 9 产物 glob 与 Task 10 签名路径使用同一 `Packrinth` 命名；`PRODUCT_NAME` 契约在 Task 9 Step 3 定义、Task 10 Step 1 消费，命名一致。