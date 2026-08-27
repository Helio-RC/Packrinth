# pnpm 指令速查

> 均从仓库根目录运行。工作区构建由 Turborepo（`turbo.jsonc`）编排，构建粒度由 pnpm workspace 与 turbo `--filter` 控制。

## 根目录指令

| 指令 | 命令内容 | 用途 |
| ------ | ------ | ------ |
| `pnpm app:dev` | `turbo run dev --filter=@modrinth/app` | 启动桌面应用开发（`tauri dev --features export-app-events`，含 Rust + 前端热重载） |
| `pnpm app:build` | `turbo run build --filter=@modrinth/app` | 打包桌面应用（`tauri build`） |
| `pnpm build` | `turbo run build --continue` | 构建所有包（失败继续） |
| `pnpm lint` | `turbo run lint lint:ancillary --continue` | 全工作区 lint + prettier 检查 |
| `pnpm lint:ancillary` | `prettier --check .github *.*` | 仅 prettier（CI/仓库文件） |
| `pnpm test` | `turbo run test --continue` | 全工作区测试 |
| `pnpm fix` | `turbo run fix fix:ancillary --continue` | 自动修复（clippy/eslint autofix + fmt + prettier） |
| `pnpm fix:ancillary` | `prettier --write .github *.*` | prettier 自动写回 |
| `pnpm ci` | `turbo run lint test --continue` | CI 入口（lint + test） |
| `pnpm prepr` | `turbo run prepr --continue` | 全部包 prepr；**开 PR 前的收尾**（跑 fix + i18n 提取/清理） |
| `pnpm prepr:frontend` | `turbo run prepr --filter=@modrinth/app-frontend` | 仅 app-frontend |
| `pnpm prepr:frontend:app` | `turbo run prepr --filter=@modrinth/app-frontend` | 同上，**前端提交前推荐命令** |
| `pnpm prepr:frontend:lib` | `turbo run prepr --filter=@modrinth/ui ...`（ui/assets/blog/api-client/utils/tooling-config） | 共享库 prepr |
| `pnpm storybook` | `pnpm i18n:coverage && pnpm --filter @modrinth/ui storybook` | 启动 UI Storybook |
| `pnpm build-storybook` | `pnpm i18n:coverage && pnpm --filter @modrinth/ui build-storybook` | 构建 Storybook |
| `pnpm icons:add` | `pnpm --filter @modrinth/assets icons:add` | 向 assets 添加图标 |
| `pnpm i18n:coverage` | `scripts/run.mjs coverage-i18n --write ...` | 生成语言覆盖度文件（展示语言可选性） |
| `pnpm changelog:collect` | `scripts/run.mjs collect-changelog` | 从各包 CHANGELOG 合并数据（blog/发布笔记用） |
| `pnpm changelog:combine-for-app` | `scripts/run.mjs build-theseus-release-notes` | 生成发布说明（发布工作流调用） |
| `pnpm scripts <name>` | `scripts/run.mjs <name>` | 运行 scripts/ 下任意 TS 脚本（如 `i18n-icu-contract prune-local`） |

## 子包常用指令

### @modrinth/app（apps/app，Tauri 壳）

| 指令 | 用途 |
| ------ | ------ |
| `pnpm --filter @modrinth/app tauri dev --features export-app-events` | 开发运行 |
| `pnpm --filter @modrinth/app tauri build` | 生产打包（默认使用 apps/app/tauri.conf.json；发布走 `--config tauri-release.conf.json`） |
| `pnpm --filter @modrinth/app test` | cargo nextest（全部 target，--no-fail-fast） |
| `pnpm --filter @modrinth/app lint` | fmt --check + clippy（含 updater feature） |
| `pnpm --filter @modrinth/app fix` | clippy --fix + fmt |

### @modrinth/app-frontend（apps/app-frontend，Vue 3）

| 指令 | 用途 |
| ------ | ------ |
| `pnpm --filter @modrinth/app-frontend dev` | 独立 vite dev server（不经 Tauri） |
| `pnpm --filter @modrinth/app-frontend build` | vue-tsc --noEmit + vite build |
| `pnpm --filter @modrinth/app-frontend lint` | eslint + prettier --check |
| `pnpm --filter @modrinth/app-frontend fix` | eslint --fix + prettier --write |
| `pnpm --filter @modrinth/app-frontend tsc:check` | 仅类型检查（vue-tsc --noEmit） |
| `pnpm --filter @modrinth/app-frontend test` | vue-tsc --noEmit（类型作为测试门槛） |
| `pnpm --filter @modrinth/app-frontend intl:extract` | formatjs 提取上游字面量 → `src/locales/en-US/index.json`（忽略 AI 工作台目录） |
| `pnpm --filter @modrinth/app-frontend intl:extract-packrinth` | formatjs 提取 Packrinth 自有 UI（`components/ai`、`AiWorkbench.vue`）→ `src/locales-packrinth/en-US/index.json` |
| `pnpm --filter @modrinth/app-frontend intl:prune-local` | 移除翻译文件中失效 key |

### @modrinth/app-playground（apps/app-playground，Rust 测试程序）

| 指令 | 用途 |
| ------ | ------ |
| `pnpm --filter @modrinth/app-playground dev` | `cargo run` |
| `pnpm --filter @modrinth/app-playground build` | `cargo build --release` |
| `pnpm --filter @modrinth/app-playground test` | cargo nextest |
| `pnpm --filter @modrinth/app-playground lint` | fmt + clippy |

## 过滤语法

- `pnpm --filter @modrinth/app dev`：只跑该包的脚本（不跑依赖链）。
- `turbo run build --filter=@modrinth/app`：只构建该包（含其依赖的构建，由 turbo 图调度）。
- 不用 filter 时 `turbo run X` 作用于所有定义了 X 的包。

## 推荐流程

1. 开发：`pnpm app:dev`
2. 本地检查：`pnpm lint && pnpm test`
3. 提交前：`pnpm prepr:frontend:app`（前端）或 `pnpm prepr`（全部）
4. 构建验证：`pnpm app:build`
