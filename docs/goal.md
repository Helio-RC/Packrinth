# AI 工作台技术规格与开发计划书（完整版·含风险预案）

**项目名称**：Packrinth（基于 Modrinth App 二次开发）
**文档版本**：3.11
**状态**：✅ 最终确认（含风险预案，RAG 向量部分暂缓；已据 2026-08-25 联网核查修正，并据设计评审补充进度协议、取消机制（含超时兜底与窗口关闭取消）、并发锁管理（含重试）、上下文压缩、布局持久化、API Key 加密与解密流程、技能热加载（含降级）与校验失败处理、BM25 增量、分页加载、模拟崩溃注入等；**按产品未上线现状，移除数据库迁移脚本、接口版本化兼容、明文→加密迁移等过度前瞻设计**，详见 §2.1/§3.3/§3.4/§5.1/§6.4/§7.2/§7.4/§8.1/§8.2/§9.3/§11）
**目标读者**：AI Agent 开发团队

---

## 目录

1. 项目概述与技术基线
2. 代码入侵点与分支策略
3. 目录结构
4. DAG 驱动的并行开发计划
5. UI/UX 设计体系（Modrinth 风格可调布局工作台）
6. AI 模型开放能力接口（原子工具 + UI 手动调用）
7. 能力扩展框架
8. 配置与数据模型（含对话持久化）
9. 测试策略与环境
10. 术语表
11. 风险与应对预案

---

## 一、项目概述与技术基线

### 1.1 项目定义

基于 Modrinth App（代号 Theseus）进行二次开发，移除版权内容和广告后，深度集成 AI 能力，打造 AI 全程参与的 Minecraft Mod Pack 制作器。核心差异化在于 AI 工作台——采用类 IDE 工作区布局，视觉遵循 Modrinth App 现有设计体系。用户通过自然语言或命令面板与 AI 交互，完成模组安装、配置修改、游戏内容定制及自动排障。同时，AI 调用的所有原子工具均提供图形化操作入口，用户可绕过 AI 直接手动执行，操作体验与 Modrinth 正常模式一致。

### 1.2 技术栈确认

| 层级 | 技术 | 代码位置 |
| ------ | ------ | ---------- |
| 桌面外壳 | Tauri 2.x (Rust) | `apps/app/` |
| 核心游戏逻辑 | theseus 库 (Rust) | `packages/app-lib/` |
| 前端 UI | Vue 3 + TypeScript + Pinia | `apps/app-frontend/` |
| 包管理 | workspaces（不绑定具体工具） | 根目录 |
| IPC 通信 | Tauri 类型安全桥接 | `apps/app/` |
| AI 接入 | Rust crate（见下方决策） | `Cargo.toml` |
| 对话持久化 | SQLite（`rusqlite`） | `Cargo.toml` |
| 全文检索 | tantivy (BM25) | `Cargo.toml` |
| HTML 解析 | scraper + html2md | `Cargo.toml` |
| 版本控制 | git2 | `Cargo.toml` |
| 向量检索（暂缓） | ~~candle 或外部 Embedding API~~ | ~~`Cargo.toml` (optional)~~ |

> **不重复造轮子原则**：
>
> - **AI 提供商接入**：采用专用 crate 直接对接各协议——`async-openai`（OpenAI 兼容，可覆盖 DeepSeek / Ollama / Custom / 任意 OpenAI 兼容端点）+ `anthropic-sdk-rust`（Anthropic）。各 provider 实现 `AiProvider` trait，逻辑完全可控、可离线、无外部 manifest 依赖。`ai-lib-rust`（"AI-Protocol" 运行时，依赖 GitHub 拉取的 provider manifest 且 `AiClient` 非 `Clone`）降级为可选/备选，仅在显式指定本地 manifest 时使用；计划的 `providers/openai.rs`、`anthropic.rs`、`ollama.rs` 结构保持不变。
> - 对话历史存储使用 SQLite（`rusqlite`），避免自建文件格式，支持事务、索引和高效查询。
> - **数据目录**：统一使用 theseus 解析出的数据目录（通过 `State::get().directories`，本仓库已依赖 `directories`(v6)/`dirs`(v6)），前端 Modrinth 实例数据已位于该目录下，AI 工作台子目录（`<data_dir>/ai-workshop/...`）在此基础之上运行时拼接，保持一致性；不混用 Tauri `app_data_dir()`，避免路径分裂。
> - **Rust 工具链**：工作区为 **edition = "2024"、rust-version = "1.90.0"**，新增 crate 须兼容。

### 1.3 上游项目信息

- 仓库地址：<https://github.com/modrinth/code>
- Fork 参考：Noctrinth（功能扩展）、Migurinth（隐私友好）
- 分支策略：`upstream/main`（只读） → `main` → `develop`（开发）

---

## 二、代码入侵点与分支策略

### 2.1 5 处代码入侵点

> 注：原计划为 6 处，经与 Modrinth `code` 仓库实际结构核对后修正为 5 处——`apps/app` 是二进制 crate（`theseus_gui`，无 `lib.rs`），前端路由为 `routes.js`（非 `router.ts`），主导航由 `App.vue` 的 `NavButton` 渲染（无 `Nav.vue`），Tauri Commands 沿用 `api/*.rs` 的 `init()` 插件模式在 `main.rs` 串联。
>
> **模块接线**：`main.rs` 中 `mod ai_workshop;` 声明的是 crate 根模块，`ai_workshop/` 目录须含 `mod.rs` 作为入口；`api/ai_workshop.rs` 通过 `crate::ai_workshop::init()` 引用该根模块（在文件顶部 `use crate::ai_workshop;` 引入即可，无需修改 `api/mod.rs` 做 `pub use`）。⚠️ `ai_workshop/mod.rs` 中的 `init()` 必须声明为 `pub fn init() -> tauri::plugin::TauriPlugin<W>`（与其他 `api/*.rs` 的 `init()` 返回类型一致），否则 `api/ai_workshop.rs` 跨模块调用会编译失败。

所有新增代码集中在 `apps/app/src/ai_workshop/`，对原生文件的修改控制在以下 5 处：

| # | 文件 | 修改内容 | 注释标记 |
| --- | ------ | ---------- | ---------- |
| 1 | `apps/app/src/main.rs` | 增加 `mod ai_workshop;`（声明模块，因本工程为二进制 crate，无 `lib.rs`）+ 在 builder 链中追加 `.plugin(api::ai_workshop::init())` | `// === AI-WORKSHOP START/END ===` |
| 2 | `apps/app/src/api/ai_workshop.rs`（新增） | 以 Tauri 插件形式 `init()` 注册 AI Tauri Commands（沿用 `api/*.rs` 的 `invoke_handler(tauri::generate_handler![...])` 模式） | 同上 |
| 3 | `apps/app-frontend/src/routes.js` | 在 `routes` 表中增加 `/ai-workbench` 路由（先于 `pages/index` 注册 `AiWorkbench` 页面） | 同上 |
| 4 | `apps/app-frontend/src/App.vue` | 在左侧导航区（`NavButton` 组件）增加 AI 工作台入口 | 同上 |
| 5 | `apps/app/Cargo.toml` & `apps/app-frontend/package.json` | 增加 AI 工作台所需依赖 | 同上 |

> **说明**：依赖管理文件（Cargo.toml 和 package.json）不可避免需要修改，因此列为第 5 处入侵点。日志捕获通过 Tauri 侧进程输出重定向实现，不修改 theseus 库。

### 2.2 分支同步流程

```bash
git fetch upstream
git checkout main && git rebase upstream/main
git checkout develop && git rebase main
```

冲突仅出现在 5 个入侵点，手动解决时间预计 < 5 分钟。

> ⚠️ 上游变更风险应对：由于 upstream 频繁更新，若入侵点所在文件发生重大重构，需及时更新占位代码。

---

## 三、目录结构

### 3.1 Rust 后端（`apps/app/src/`）

```
apps/app/src/
├── main.rs                    # 【入侵点1：声明 mod ai_workshop; 并串联 .plugin(api::ai_workshop::init())】
├── api/mod.rs                 # 模块聚合（ai_workshop 命令注册见 api/ai_workshop.rs）
├── ai_workshop/               # 【新增根目录】
│   ├── mod.rs                 # 模块入口：init() 钩子
│   ├── config.rs              # 配置管理（读取独立 ai_workshop.json 或数据目录下 ai-workshop/config.json，见 §8.1）
│   ├── providers/             # AI 提供商适配层（使用现成 crate）
│   │   ├── mod.rs
│   │   ├── trait.rs           # AiProvider trait（统一抽象）
│   │   ├── openai.rs          # OpenAI 实现
│   │   ├── anthropic.rs       # Anthropic 实现
│   │   ├── ollama.rs          # Ollama 实现
│   │   ├── ......
│   │   ├── factory.rs         # ProviderFactory
│   │   └── mock.rs            # MockProvider
│   ├── inference/             # 推理引擎
│   │   ├── mod.rs
│   │   ├── engine.rs          # 多轮 tool_calls 循环（含最大轮次限制）
│   │   └── context.rs         # 上下文管理
│   ├── chat_history/          # 【新增】对话持久化
│   │   ├── mod.rs
│   │   ├── db.rs              # SQLite 连接管理与建表（建表语句见 §3.4）
│   │   ├── repository.rs      # 消息/会话的 CRUD 操作
│   │   └── models.rs          # 数据结构映射
│   ├── tools/                 # 【L1 原子工具】
│   │   ├── mod.rs
│   │   ├── registry.rs        # ToolRegistry（供 AI 引擎和前端 UI 共用）
│   │   ├── trait.rs           # AtomicTool trait
│   │   ├── mod_ops.rs         # 模组增删改查
│   │   ├── config_ops.rs      # 配置文件读写
│   │   └── script_gen.rs      # 脚本生成
│   ├── toolchain/             # 【L2 可执行工具链】
│   │   ├── mod.rs
│   │   ├── registry.rs        # ToolchainRegistry
│   │   ├── trait.rs           # ExecutableToolchain trait
│   │   └── builtin/           # 内置工具链
│   │       ├── kubejs_gen.rs
│   │       ├── ct_gen.rs
│   │       ├── ftb_recipe.rs
│   │       ├── mod_config.rs
│   │       └── pack_export.rs
│   ├── skills/                # 【L3 技能】
│   │   ├── mod.rs
│   │   ├── loader.rs          # 扫描 + 解析 skill.toml（含安全校验）
│   │   ├── matcher.rs         # 关键词匹配（大小写不敏感全词匹配，可选正则）+ 优先级排序（见 §4.5 D.2）
│   │   └── injector.rs        # 注入 System Prompt
│   ├── knowledge/             # 知识检索（仅 BM25，向量暂缓）
│   │   ├── mod.rs
│   │   ├── source.rs          # KnowledgeSource trait
│   │   ├── bm25.rs            # Tantivy 实现
│   │   ├── router.rs          # 检索路由
│   │   ├── crawler.rs         # 爬虫 (scraper) – 限定域名白名单
│   │   └── chunker.rs         # 分块
│   ├── git_ops.rs             # Git (git2)
│   ├── mcp_client.rs          # MCP 客户端（含健康检查）
│   ├── troubleshooter.rs      # 日志环形缓冲区（容量可配置，支持落盘持久化）+ 排障
│   ├── context_guard.rs       # 上下文窗口溢出保护（含迭代次数限制）
│   └── ui_commands.rs         # 将原子工具封装为 Tauri Commands 供前端 UI 调用
```

> ⚠️ 每个文件夹下都应有README.md，说明模块职责、接口、依赖关系和测试策略。

### 3.2 前端 Vue 3（`apps/app-frontend/src/`）

```
apps/app-frontend/src/
├── pages/AiWorkbench.vue      # 【新增】主页面（可调整布局容器）
├── components/ai/             # 【新增】
│   ├── layout/                # 布局系统
│   │   ├── WorkbenchLayout.vue # 可拖拽/可调整的布局容器
│   │   ├── ActivityBar.vue    # 活动栏 (可拖动位置)
│   │   ├── SidePanel.vue      # 侧边面板 (宽度可调整)
│   │   ├── MainArea.vue       # 主区域 (可拆分/合并)
│   │   ├── BottomPanel.vue    # 底部面板 (高度可调整)
│   │   └── StatusBar.vue      # 顶部状态栏 (32px)
│   ├── chat/                  # 对话相关
│   │   ├── ChatMessage.vue
│   │   ├── ChatInput.vue
│   │   ├── ChatHistory.vue    # 历史会话列表（从 SQLite 加载）
│   │   └── ToolCard.vue
│   ├── sidebar/               # 侧边面板内容
│   │   ├── ConsoleView.vue
│   │   ├── FileTree.vue
│   │   ├── KnowledgeView.vue
│   │   ├── SkillsView.vue     # 技能管理面板
│   │   └── ToolsView.vue      # 手动工具面板（原子工具 UI 入口）
│   ├── preview/
│   │   ├── PreviewPanel.vue
│   │   ├── ConfigEditor.vue
│   │   └── DiffView.vue
│   └── bottom/
│       ├── LogViewer.vue
│       ├── ToolOutput.vue
│       └── TroubleshootReport.vue
├── stores/aiWorkshop.ts       # Pinia（含布局状态、当前会话状态）
├── lib/ai/
│   ├── client.ts              # AI 通信客户端
│   ├── tools.ts               # 工具调用的前端封装
│   ├── history.ts             # 对话历史 API 客户端（调用 Tauri Commands）
│   ├── websocket.ts
│   └── types.ts
└── routes.js                  # 【入侵点3：增加 /ai-workbench 路由】
```

### 3.3 技能存储目录（用户侧）

```
{Modrinth_default_data_dir}/ai-workshop/skills/
├── ftb-recipes/
│   ├── skill.toml
│   └── guide.md
├── kubejs-advanced/
│   ├── skill.toml
│   └── guide.md
├── create-mod/
│   ├── skill.toml
│   └── guide.md
└── user-uploaded/
    └── my-skill/
        ├── skill.toml
        └── guide.md
```

> ⚠️ 技能安全限制（`loader.rs` 实现，来自设计评审）：
>
> - **路径遍历防护**：使用 `std::fs::canonicalize` 将用户提供的路径与 `base_path` 规范化后比较，防止 `../` 绕过；在 `loader.rs` 中封装 `safe_path` 函数，统一应用于所有文件操作。**导入豁免（2026-08-28 修订）**：`import_skill` 来源可为任意已存在的目录（canonicalize + 存在性/目录校验，不做前缀包含限制），目录名取规范化来源的叶名，目标始终为 `base_path.join(dir_name)`，不会逃逸 base_path；内部扫描/读取仍严格受 base_path 限制。
> - **内容净化**（封装于独立 `sanitizer.rs`，便于单测）：第一层用 `pulldown-cmark` 解析为 AST，拒绝包含 HTML 块或 `html` 标签的事件；第二层对允许的 AST 渲染为 HTML 后用 `ammonia` 清理 `<script>`、`on*` 事件、`javascript:` 链接；第三层限制链接仅允许 `http`/`https` 协议，禁止 `file://` 或 `data:`。明确不支持嵌入脚本或外部资源。

### 3.4 对话历史存储位置（用户侧）

```
{Modrinth_default_data_dir}/ai-workshop/chat_history/
└── chat.db                  # SQLite 数据库文件
```

数据库表结构（初始设计）：

```sql
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,           -- UUID
    title TEXT NOT NULL,
    instance_id TEXT,              -- 关联实例（可选）
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,            -- 'user' | 'assistant' | 'tool'
    content TEXT NOT NULL,         -- 消息正文或工具结果
    tool_calls TEXT,               -- JSON 序列化的工具调用信息（可选）
    tool_call_id TEXT,             -- 关联的工具调用 ID（可选）
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_messages_conversation ON messages(conversation_id, created_at);
CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC);
```

> **对话历史分页加载（来自设计评审）**：`get_conversation` 接口支持 `offset` / `limit` 分页参数（按 `created_at` 升序，默认每页 50 条），避免长对话（数千条消息）一次性加载拖慢前端；前端初始仅加载最近 50 条，用户滚动到顶部时触发"加载更多"拉取更早消息，每个会话的加载状态独立管理，切换会话不重复请求。

---

## 四、DAG 驱动的并行开发计划

### 4.1 阶段 0：基础基建（唯一串行阻塞点）

| 任务 ID | 子任务 | 操作 | 产出 |
| --------- | -------- | ------ | ------ |
| 0.1 | Fork 与环境 | Fork 仓库，安装依赖，创建 `develop` 分支 | 开发环境就绪 |
| 0.2 | 目录骨架 | 创建 `ai_workshop/` 和 `components/ai/` 目录；5 个入侵点植入占位代码 | 目录结构 |
| 0.3 | 配置系统 | 新增独立 `ai_workshop.json`（打包进 resources）或数据目录下 `ai-workshop/config.json`；实现 `config.rs` 加载/保存（**不写入 `tauri.conf.json`**，见 §8.1） | 配置可读写 |
| 0.4 | Feature Flags | `log_lines`(默认500)、`mock_enabled`(默认false) | 开关就绪 |
| 0.5 | 空白页面 | `AiWorkbench.vue` 可访问，显示“AI 工作台开发中” | 路由打通 |
| 0.6 | SQLite 初始化 | 引入 `rusqlite`，在 `<data_dir>/ai-workshop/chat_history/` 创建数据库与表结构（建表语句见 §3.4） | 对话历史数据库就绪 |

**完成标志**：应用正常启动，AI 工作台页面可访问，SQLite 数据库可正常读写。

### 4.2 并行流 A：AI 核心引擎（依赖：阶段 0）

| 任务 ID | 子任务组 | 操作 |
| --------- | ---------- | ------ |
| A.1 | Provider 抽象层 | 定义 `AiProvider`（chat、stream、tool_call） |
| A.2 | Mock Provider | 实现 `MockProvider`；预置 20+ JSON 案例集 |
| A.3 | 推理引擎 | `InferenceEngine`（多轮 tool_calls 循环，最大迭代次数 5 轮）；`ToolRegistry` |
| A.4 | Tauri Commands | 暴露 `ai_chat`、`ai_stream`、`ai_confirm_tool` |
| A.5 | 对话持久化集成 | 推理引擎调用历史仓储，将每轮消息和工具结果写入 SQLite |

### 4.3 并行流 B：前端 UI 交互层（依赖：阶段 0）

| 任务 ID | 子任务组 | 操作 |
| --------- | ---------- | ------ |
| B.1 | 布局骨架与可调整系统 | 实现 `WorkbenchLayout.vue`；支持面板拖拽调整大小、显示/隐藏、活动栏位置切换、面板拆分/合并；布局状态持久化到 Pinia + localStorage（运行时状态）；配置文件仅提供初始默认值 |
| B.2 | 状态栏与仪表盘 | 顶部状态栏（含 Token 消耗实时显示）；侧边栏控制台视图（Mod 数量、磁盘占用、Git 分支）+ 核心开关区 |
| B.3 | 对话流主面板 | 消息列表（Markdown）；流式渲染；`ToolCard`；输入框 + `/` 命令面板 |
| B.4 | 预览与配置编辑器 | 动态预览；Monaco Editor；Diff 视图 |
| B.5 | 底部面板 | 日志查看器；工具输出追踪；排障报告 |
| B.6 | 知识库与技能 | 知识库列表；技能浏览器（启用/禁用/优先级拖拽，首次启动默认全部禁用） |
| B.7 | 手动工具面板 | `ToolsView.vue`：展示所有原子工具，提供表单/按钮式操作，调用前端封装的 Tauri Commands |
| B.8 | 状态与通信 | Pinia Store；WebSocket；localStorage 持久化 |
| B.9 | 对话历史 UI | `ChatHistory.vue`：加载历史会话列表，支持切换、删除、重命名；从 SQLite 加载消息记录并渲染 |

**🔗 汇合点 1：AI 对话就绪**

| 任务 | 操作 |
|------|------|
| M1-1 | 前端 `AiClient` 对接 `ai_stream`，Tauri `ipc::Channel` 事件流渲染（2026-08-28 修订：实现采用 Channel 而非 HTTP SSE——Tauri 2 无内置 SSE 端点，启本地 HTTP 端口属过度工程；`lib/ai/websocket.ts` 保留为 MCP/远程流预留） |
| M1-2 | Mock 模式验证基础对话 |
| M1-3 | 工具调用卡片显示 `tool_calls`，用户确认/拒绝 |
| M1-4 | 对话历史可保存和恢复（验证 SQLite 写入与读取） |

### 4.4 并行流 C：原子工具与文件系统（依赖：汇合点 1）

| 任务 ID | 子任务组 | 操作 |
| --------- | ---------- | ------ |
| C.1 | 模组操作 Tools | `search_mods`、`install_mod`、`remove_mod`、`update_mod`、`list_mods`（直接调用 theseus API） |
| C.2 | 配置读写 Tools | 格式检测；`read_config`、`write_config`、`rollback_config` |
| C.3 | 脚本生成 Tools | `generate_kubejs_script`、`generate_crafttweaker_script` |
| C.4 | Git 版本控制 | git2；init、commit、log、checkout、branch |
| C.5 | 日志环形缓冲区 | `LogBuffer`；通过 Tauri 侧进程输出重定向捕获游戏日志，不修改 theseus 库；**容量可配置**（默认 `log_lines=500`，长运行实例可调大），并支持**周期性落盘**到 `<data_dir>/ai-workshop/logs/` 以应对崩溃后分析；**落盘频率**：默认每新增 `log_lines/10` 条（即 50 条）或每 120 秒落盘一次，可由配置调整，平衡 I/O 开销与崩溃丢失量 |
| C.6 | 工具前端封装 | 将上述工具封装为 Tauri Commands，供 `ToolsView.vue` 调用 |

### 4.5 并行流 D：知识增强与技能（依赖：汇合点 1）

| 任务 ID | 子任务组 | 操作 |
| --------- | ---------- | ------ |
| D.1 | 技能加载器 | 扫描 `{Modrinth_default_data_dir}/ai-workshop/skills/`；解析 `skill.toml` + `guide.md`（含安全校验） |
| D.2 | 技能匹配器 | 关键词匹配（**大小写不敏感全词匹配**，可选正则）；**优先级排序**规则：先按 `skill.toml` 中 `priority` 降序，同优先级按匹配命中数降序、再按名称稳定排序；自动/手动加载（默认全部禁用） |
| D.3 | BM25 知识检索 | tantivy BM25；暂不引入向量检索，后续版本再扩展；**增量更新**：为每个 `KnowledgeSource` 记录 mtime 作为第一道过滤，仅当 mtime 变化时才计算内容哈希确认，仅内容变化的源重新索引（Tantivy `IndexWriter.add_document`），全量重建耗时长故在后台进行、不阻塞主线程（见 §8.1 索引刷新） |
| D.4 | 内容获取与解析 | scraper + html2md；限定域名白名单；智能分块（≤512 tokens） |
| D.5 | 融合调度 | 上下文构建管道（RAG Top-3 + 技能 Top-3）；窗口看门狗 |
| D.6 | 工具链框架 | `ExecutableToolchain` trait；注册 `KubeJsGen`、`FtbRecipe` 等 |

**🔗 汇合点 2：AI 代理能力就绪**

| 任务 | 操作 |
|------|------|
| M2-1 | 将 `ToolRegistry` + `KnowledgeRouter`(BM25) + `SkillLoader` + `ToolchainRegistry` 注入 `InferenceEngine` |
| M2-2 | 测试完整链路：“装个 JEI 并加个配方” → 多步操作闭环 |
| M2-3 | 前端操作卡片展示多步进度 |

### 4.6 并行流 E：高级场景层（依赖：汇合点 2）

| 任务 ID | 子任务组 | 操作 |
| --------- | ---------- | ------ |
| E.1 | 自动排障闭环 | 游戏异常退出触发；提取环形缓冲区；AI 分析；自动修复前弹出用户确认 |
| E.2 | MCP 客户端集成 | 独立子进程；stdio JSON-RPC；工具发现并注册（默认 `enabled: false`） |
| E.3 | 应用设置页整合 | “AI”选项卡；提供商选择（无默认，用户自行配置）；API Key；日志行数；技能管理 |
| E.4 | 布局自定义设置 | 提供“重置布局”按钮；导出/导入布局配置文件 |
| E.5 | 历史数据管理 | 提供清理旧对话、导出对话记录、数据库备份/恢复功能 |

### 4.7 并行流 T：测试基建（贯穿全流程）

| 任务 ID | 子任务 | 触发时机 |
| --------- | -------- | ---------- |
| T.1 | Mock 案例库（20+ JSON） | 与流 A 并行 |
| T.2 | 单元测试（Git/缓冲区/配置解析/安全校验/工具执行/对话持久化） | 流 C/D 模块完成即写 |
| T.3 | E2E 测试（tauri-driver + Playwright） | 汇合点 2 之后 |
| T.4 | CI/CD（GitHub Actions） | 阶段 0 完成后 |

---

## 五、UI/UX 设计体系（Modrinth 风格可调布局工作台）

> **设计原则**：工作台布局参考 VS Code 的可调整性，但视觉风格严格遵循 Modrinth App 的现有设计语言。所有组件使用 Modrinth 的设计令牌，确保与主应用无缝融合。

### 5.1 布局架构（可调整）

默认布局如下，但所有面板均可通过拖拽调整大小、移动位置、显示/隐藏，用户可自由定制。

```
+--------+----------+---------------------------+--------+
| 活动栏   | 侧边面板  |  主区域（可拆分/合并）     | 右侧   |
| (48px)  | (可调宽)  |  ┌─────────┬────────────┐ | (可选) |
|  [聊]   | 聊天历史   |  │ 对话流    │ 配置预览     | |       |
|  [文]   | 实例树     |  │ (可调)    │ (可调)       | |       |
|  [知]   | 技能列表  |  └─────────┴────────────┘ |       |
|  [工]   | 手动工具   |                            |       |
|  [控]   | 控制台     |  +--------------------------+ |       |
|  [设]   |           |  | 底部面板 (高度可调)       | +-------+
+--------+----------+  +--------------------------+
```

**可调整特性**：

- 活动栏可切换左右位置（左侧或右侧）。
- 侧边面板宽度可拖拽调整（最小 200px，最大 600px），可折叠。
- 主区域可水平/垂直拆分多个视图，拖拽分隔线调整比例，支持标签页拖拽合并。
- 底部面板高度可调整，支持折叠为标题栏。
- 所有面板均可独立显示/隐藏。
- 布局状态自动保存，支持一键重置为默认布局。
- **默认布局与跨设备一致**：默认布局从后端 `config.json` 的 `layout` 节读取（见 §8.1）；运行时修改写入 localStorage（Pinia 持久化）；提供"保存为默认"按钮将当前布局写回 `config.json`，使切换机器/重装后布局一致。仅导出/导入布局文件不足以覆盖"重置布局"与"切换机器"场景，故以 `config.json` 为默认真源。
- **"重置布局"与"保存为默认"的关系（来自设计评审）**："重置布局"= 从 `config.json` 的 `layout` 节重新加载（即使用户曾修改过默认值），"保存为默认"= 将当前运行时布局写回 `config.json`；另提供"恢复出厂布局"选项（忽略用户写回的默认值，恢复硬编码初始布局）。UI 中"重置布局"按钮下以下拉区分"重置为默认布局（从配置文件）"与"恢复出厂设置（忽略配置文件）"，避免用户混淆。三者职责分明。

### 5.2 核心组件清单

| 组件 | 文件 | 功能 |
| ------ | ------ | ------ |
| 布局容器 | `WorkbenchLayout.vue` | 管理面板拖拽、调整大小、显示/隐藏、持久化 |
| 活动栏 | `ActivityBar.vue` | 垂直图标导航（聊天/文件/知识/工具/控制台/设置），可拖动切换左右 |
| 侧边面板 | `SidePanel.vue` | 根据活动栏动态切换内容，宽度可调 |
| 聊天历史 | `ChatHistory.vue` | 展示会话列表，支持新建、切换、删除、重命名 |
| 手动工具面板 | `ToolsView.vue` | 展示所有原子工具，提供表单/按钮操作（与 AI 调用同一后端逻辑） |
| 控制台视图 | `ConsoleView.vue` | 数据仪表盘 + 核心开关区 |
| 技能视图 | `SkillsView.vue` | 技能管理（启用/禁用/优先级拖拽） |
| 主区域 | `MainArea.vue` | 可水平/垂直拆分，支持拖拽 |
| 对话流 | `ChatMessage.vue` | Markdown 渲染 + 代码高亮 + 流式打字机 |
| 输入框 | `ChatInput.vue` | 多行文本 + `/` 命令面板 |
| 工具卡片 | `ToolCard.vue` | 显示工具调用参数、确认/拒绝按钮、执行进度 |
| 配置编辑器 | `ConfigEditor.vue` | Monaco Editor |
| Diff 视图 | `DiffView.vue` | 修改前后对比 |
| 日志查看器 | `LogViewer.vue` | 日志着色、暂停/滚动 |
| 排障报告 | `TroubleshootReport.vue` | AI 诊断 + 一键修复 |
| 状态栏 | `StatusBar.vue` | 实例名 + MC 版本 + AI 连接状态 + Token 消耗 |

### 5.3 核心开关区（控制台视图内）

| 开关 | 默认值 | 风险等级 | 说明 |
| ------ | -------- | ---------- | ------ |
| AI 主开关 | ON | 低 | 全局启用/禁用 AI 功能 |
| Mock 模式 | OFF | 低 | 测试用，不调用真实 API |
| 自动排障 | ON | 中 | 游戏崩溃自动触发 AI 分析（修复前需用户确认） |
| 日志等级 | INFO | 低 | DEBUG/INFO/WARN/ERROR |

---

## 六、AI 模型开放能力接口（原子工具 + UI 手动调用）

### 6.1 原子工具接口清单

> 所有原子工具不仅供 AI 引擎调用，也通过 `ui_commands.rs` 封装为 Tauri Commands，前端 `ToolsView.vue` 可直接调用，为用户提供手动操作入口。
>
> **暂缓项（2026-08-28 修订）**：`get_config_schema` 与 `validate_script` 暂缓实现（无 theseus 底层 API 可包装，完整实现需真实 JSON-Schema 规则库/JS 解析器）；待后续版本完整实现。工具链由 `execute_toolchain` 命令执行（`list_toolchains` 查询）。

#### 模组管理

| 接口名称 | 描述 | 参数 | 返回值 |
| ---------- | ------ | ------ | -------- |
| `search_mods` | 搜索模组 | `query` (string), `limit` (int, 默认10), `loader` (string, 可选) | `ModInfo[]` |
| `get_mod_details` | 获取模组详情 | `mod_id` (string) | `ModDetail` |
| `install_mod` | 安装模组 | `mod_id`, `instance_id`, `version` (可选) | `InstallResult` |
| `remove_mod` | 删除模组 | `mod_id`, `instance_id`, `keep_config` (bool) | `RemoveResult` |
| `update_mod` | 更新模组 | `mod_id`, `instance_id` | `UpdateResult` |
| `list_installed_mods` | 列出已安装 | `instance_id` | `InstalledMod[]` |
| `resolve_dependencies` | 解析依赖 | `mod_ids` (string[]), `instance_id` | `DependencyReport` |

#### 配置文件操作

| 接口名称 | 描述 | 参数 | 返回值 |
| ---------- | ------ | ------ | -------- |
| `read_config` | 读取配置 | `instance_id`, `mod_id`, `config_path` | `ConfigContent` |
| `write_config` | 写入配置 | `instance_id`, `mod_id`, `config_path`, `value`, `backup` | `WriteResult` |
| `rollback_config` | 回滚配置 | `instance_id`, `mod_id`, `config_path` | `RollbackResult` |
| `list_configs` | 列出配置文件 | `instance_id`, `mod_id` | `ConfigFile[]` |
| `diff_config` | 对比差异 | `instance_id`, `mod_id`, `config_path` | `DiffResult` |
| `get_config_schema` | 获取 JSON Schema | `mod_id`, `config_path` | `JSONSchema` |

#### 脚本生成

| 接口名称 | 描述 | 参数 | 返回值 |
| ---------- | ------ | ------ | -------- |
| `generate_kubejs_script` | 生成 KubeJS 脚本 | `instance_id`, `script_type`, `content` | `ScriptResult` |
| `generate_crafttweaker_script` | 生成 CraftTweaker 脚本 | `instance_id`, `script_type`, `content` | `ScriptResult` |
| `validate_script` | 验证语法 | `instance_id`, `script_path`, `loader` | `ValidationResult` |

#### 实例与版本管理

| 接口名称 | 描述 | 参数 |
| ---------- | ------ | ------ |
| `list_instances` | 列出所有实例 | 无 |
| `get_instance_info` | 获取实例详情 | `instance_id` |
| `create_instance` | 创建实例 | `name`, `mc_version`, `loader`, `loader_version` |
| `duplicate_instance` | 复制实例 | `instance_id`, `new_name` |
| `delete_instance` | 删除实例 | `instance_id`, `keep_files` |
| `launch_instance` | 启动游戏 | `instance_id`, `jvm_args` |

#### Git 版本控制

| 接口名称 | 描述 | 参数 |
| ---------- | ------ | ------ |
| `git_init` | 初始化 Git | `instance_id` |
| `git_commit` | 提交变更 | `instance_id`, `message` |
| `git_log` | 提交历史 | `instance_id`, `limit` |
| `git_checkout` | 切换版本 | `instance_id`, `commit_hash` |
| `git_diff` | 查看变更 | `instance_id` |
| `git_branch` | 分支管理 | `instance_id`, `action`, `branch_name` |
| `git_status` | 仓库状态 | `instance_id` |

#### 知识检索与技能

| 接口名称 | 描述 | 参数 |
| ---------- | ------ | ------ |
| `search_knowledge` | 检索知识库（BM25） | `query`, `top_k`, `source` |
| `list_skills` | 列出所有技能 | 无 |
| `get_skill_content` | 获取技能内容 | `skill_name` |
| `enable_skill` | 启用技能 | `skill_name` |
| `force_load_skill` | 强制加载技能 | `skill_name` |
| `refresh_skills` | 刷新技能索引 | 无 |
| `import_skill` | 导入技能 | `path` |

#### 系统与排障

| 接口名称 | 描述 |
| ---------- | ------ |
| `get_ai_status` | 获取 AI 状态 |
| `analyze_crash` | 分析崩溃 |
| `get_logs_for_ai` | 获取格式化日志 |
| `suggest_fix` | 获取修复建议 |
| `apply_fix` | 应用修复（需用户确认） |

#### 对话历史管理

| 接口名称 | 描述 | 参数 |
| ---------- | ------ | ------ |
| `list_conversations` | 列出所有会话 | `instance_id` (可选)，`limit` |
| `get_conversation` | 获取会话详情（含消息列表） | `conversation_id` |
| `create_conversation` | 新建会话 | `title`, `instance_id` (可选) |
| `rename_conversation` | 重命名会话 | `conversation_id`, `new_title` |
| `delete_conversation` | 删除会话及所有消息 | `conversation_id` |
| `export_conversation` | 导出会话为 JSON 或 Markdown | `conversation_id`, `format` |
| `clear_all_conversations` | 清空所有会话（需前端二次确认） | `confirm: boolean` |

### 6.2 统一响应格式

```typescript
interface ToolResponse<T> {
  success: boolean
  data?: T
  error?: {
    code: string
    message: string
    details?: any
  }
  tool_call_id?: string
}
```

### 6.3 推理引擎限制（风险防范）

- 最大 `tool_calls` 轮次：**5 轮**
- Token 消耗实时显示，超出阈值请求确认
- 所有写操作必须经用户通过 `ToolCard` 确认

### 6.4 工具手动调用（UI 封装）

> 前端 `lib/ai/tools.ts` 定义统一的 `executeTool(name, params)` 函数，内部通过 Tauri `invoke('tool_execute', { name, params })` 调用后端。`ToolsView.vue` 根据工具暴露的**参数 Schema**（由 `ui_commands.rs` 随命令一并返回，见 §7.2）动态渲染表单，用户填写后执行，结果展示在底部面板"工具输出"标签页。
>
> **进度上报协议（来自设计评审）**：所有工具均为异步执行，须通过统一协议上报进度，避免各工具格式不一导致前端无法通用渲染。定义 `ProgressPayload { step: String, percent: Option<f32>, message: Option<String> }`；后端统一通过 `AppHandle.emit("tool-progress", payload)` 广播，前端用 `useListen("tool-progress", ...)` 监听；`ExecutionContext` 提供 `report_progress(step, percent, message)` 辅助函数，工具通过 `ctx` 调用。耗时操作（安装模组、解压、生成脚本）必须实现进度回调，前端以进度条反馈，防止 UI 冻结。
>
> **取消机制（来自设计评审）**：`ExecutionContext` 持有 `CancellationToken`，每个工具执行时传入；`ctx` 提供 `check_cancelled()` 辅助函数，工具在每次循环迭代或网络 IO 后调用，检测到取消即**立即返回**（保证已分配资源被释放/回滚，不完成当前原子操作）。整个 `execute` 在 `ExecutionContext.tool_timeout_secs`（默认 300s）下用 `tokio::time::timeout` 包裹，超时即取消并报错，作为底层 theseus API 不支持取消时的兜底；对确实无法取消的操作，在工具文档标注"部分取消能力有限"。前端侧：每次 `executeTool` 返回唯一 `task_id`，用户点击取消时前端调用独立 Tauri Command `cancel_task(task_id)`，后端通过 `HashMap<task_id, CancellationToken>` 找到对应 token 触发取消；若用户在执行中关闭 `AiWorkbench` 页面或整个应用，后端监听 `window.close_requested` 事件取消所有进行中任务（`CancellationToken.cancel()`），应用整体退出则直接终止进程。

---

## 七、能力扩展框架

### 7.1 三层能力架构总览

| 层次 | 名称 | 可执行 | 热加载 | 用户可编辑 | 存储位置 |
| ------ | ------ | -------- | -------- | ------------ | ---------- |
| L1 | 原子工具（Atomic Tools） | ✅ | ❌ | ❌ | Rust 代码 |
| L2 | 可执行工具链（Executable Toolchains） | ✅ | ❌ | ❌ | Rust 代码 |
| L3 | 技能（Skills） | ❌ | ✅ | ✅ | 文件系统 |

### 7.2 L1：原子工具（Atomic Tool）

**定义**：单一职责的最小可执行单元，直接操作文件系统、调用 theseus API、执行 Git 命令等。所有工具均实现 `AtomicTool` trait，并通过 `ToolRegistry` 注册，同时供 AI 引擎和前端 UI 调用。

核心 Trait：

```rust
#[async_trait]
pub trait AtomicTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn params_schema() -> schemars::schema::RootSchema where Self: Sized, Self::Params: JsonSchema;
    fn requires_confirmation(&self) -> bool;  // 写操作返回 true
    fn domain(&self) -> ToolDomain;
    fn is_readonly(&self) -> bool;
    type Params: ToolParams;
    type Output: ToolOutput;
    async fn execute(&self, params: Self::Params, ctx: &ExecutionContext) -> Result<Self::Output, ToolError>;
}
```

**设计约束（来自设计评审）**：

- **异步与进度**：`execute` 必须为 `async`；耗时操作（网络下载、文件解压、脚本生成）须通过 `ctx` 提供的进度回调（或 Tauri `emit`）上报阶段与百分比，前端据此渲染进度条，防止 UI 冻结。
- **参数 Schema 暴露**：`params_schema()` 返回的 `schemars::schema::RootSchema` 须由 `ui_commands.rs` 在注册命令时一并序列化为 JSON Schema 暴露给前端，使 `ToolsView.vue` 能自动渲染输入控件（字段类型、必填、默认值），无需为每个工具手写表单。
- **并发安全（来自设计评审）**：工具可能由 AI 引擎与手动面板并发触发，且多个写操作可能作用于同一实例的配置文件或模组列表。引入 `InstanceLockManager`：内部维护 `tokio::sync::Mutex<HashMap<InstanceId, Arc<tokio::sync::Mutex<()>>>>`，写类工具在 `execute` 开头调用 `ctx.acquire_write_lock(instance_id)` 获取该实例的互斥锁（读工具不获取，可并发）；若需限制并发写数，可用 `tokio::sync::Semaphore`（每实例最多 1 个写任务）。`acquire_write_lock` 须带超时（如 `tokio::time::timeout(Duration::from_secs(30), lock)`），超时返回错误由上层决定重试或放弃；**禁止在已持有锁的工具内部再次调用 `acquire_write_lock`**（文档约束 + 运行时检测，违例即 panic/错误返回），避免嵌套锁死锁。`ExecutionContext` 注入 `instance_lock_manager`（见 §8.2），工具通过 `ctx` 调用，避免文件锁（`fs2`/`fs4`）可能带来的死锁/超时挂起。锁获取超时返回的错误须明确提示"另一个操作正在修改此实例，请稍后重试"；前端可自动重试（如 3 次、间隔 2 秒），并在进度条显示"等待实例释放..."。

新增步骤：实现 trait，在 `registry.rs` 中注册，并在 `ui_commands.rs` 中封装为 Tauri Command。

### 7.3 L2：可执行工具链（Executable Toolchain）

**定义**：由多个原子工具组合成的复合流程，例如“安装模组并处理依赖”或“生成 KubeJS 脚本并写入文件”。工具链不可热加载，编译时固化，用户不可直接编辑，但可通过原子工具组合实现类似效果。核心 trait 类似 `ExecutableToolchain`，包含 `name`、`description`、`steps` 和 `execute` 方法。

### 7.4 L3：技能（Skills）

**定义**：Markdown 格式的知识文档，通过 `skill.toml` 描述元信息（名称、触发关键词、优先级），热加载，用户可编辑，用于增强 AI 上下文而非直接执行。加载、匹配和注入逻辑分别由 `loader.rs`、`matcher.rs`、`injector.rs` 实现。

> **热加载（来自设计评审）**：集成 `notify` crate 监听技能目录（`<data_dir>/ai-workshop/skills/`），文件新增/修改/删除时自动触发 `refresh_skills`，重新扫描并更新 `SkillLoader` 内部索引，使 `list_skills` / `match_skills` 立即生效，无需重启应用。**跨平台降级**：`notify` 在 macOS（FSEvent）/Linux（inotify）行为有差异，若文件监听初始化失败则记录警告并回退到手动刷新（用户点击"刷新技能"按钮），不阻塞启动。

**`skill.toml` 字段规范（来自设计评审）**：

```toml
name = "FTB Recipes"
description = "Guide for creating FTB-style recipes"
keywords = ["ftb", "recipe", "crafting"]
priority = 1
version = "1.0"
author = "user"
```

> **字段校验规则（来自设计评审）**：`loader.rs` 须校验——`priority` 范围 0~100（数值越大优先级越高）；`keywords` 至少 1 个、最多 20 个；`name` 仅允许 **Unicode 字母/数字、空格和连字符**（2026-08-28 修订：放行中文等非 ASCII 字母，中文技能名为合法需求；仍拒绝符号与控制字符）。**校验失败则跳过整个技能**（不部分加载），在 `app.log` 记录警告，并在 `SkillsView.vue` 的"加载失败的技能"列表中展示，便于用户排查（该列表已实现于 `list_skills` 返回的 `failed` 字段）。

### 7.5 扩展方式对比总结

| 特性 | 原子工具 | 可执行工具链 | 技能 |
| ------ | ---------- | -------------- | -------- |
| 用途 | 基础文件/API 操作 | 复合可执行流程 | 知识注入 |
| 用户可见 | 是（手动工具面板） | 否（底层） | 是（设置页可见） |
| 修改方式 | Rust 代码 + 编译 | Rust 代码 + 编译 | 复制/编辑文件 |
| 新增成本 | ~80 行代码 | ~100 行代码 | 写 Markdown |
| 安全风险 | 低 | 低 | 中 |

---

## 八、配置与数据模型

### 8.1 AI 配置

AI 配置**不写入 `tauri.conf.json`**——Tauri 2 的 `tauri.conf.json` 顶层仅允许 `app` / `build` / `bundle` / `plugins` / `productName` / `version` / `identifier` / `mainBinaryName` 等固定字段，自定义顶层键 `ai_workshop` 会导致构建期 schema 校验失败。改为：

- 在 `apps/app/src/` 下新增独立文件 `ai_workshop.json`（随二进制打包进 `resources`，运行时读取），或
- 首次运行时在 theseus 数据目录下生成 `ai-workshop/config.json`，由 `config.rs` 读写。

> 下方 JSON 为配置**内容结构**（字段含义不变），存放位置按上述方式处理，不再作为 `tauri.conf.json` 的顶层键。

```json
{
  "ai_workshop": {
    "enabled": true,
    "log_lines": 500,
    "mock_enabled": false,
    "max_tool_iterations": 5,
    "token_warning_threshold": 4000,
    "default_provider": null,
    "providers": {
      "openai": { "api_key": "", "model": "gpt-4", "enabled": false },
      "anthropic": { "api_key": "", "model": "claude-3", "enabled": false },
      "deepseek": { "api_key": "", "model": "deepseek-chat", "enabled": false },
      "ollama": { "base_url": "http://localhost:11434", "model": "llama3", "enabled": false },
      "custom": { "base_url": "", "api_key": "", "model": "", "enabled": false }
    },
    "knowledge": {
      "mode": "BM25",
      "bm25_index_path": "<data_dir>/ai-workshop/bm25_index",
      "allowed_domains": ["modrinth.com", "mcmod.cn", "minecraft.fandom.com", "ftbwiki.org"]
    },
    "skills": {
      "base_path": "<data_dir>/ai-workshop/skills",
      "auto_load": false,
      "max_inject_count": 3
    },
    "mcp": {
      "enabled": false,
      "command": "npx",
      "args": ["-y", "@modrinth/mcp"],
      "health_check_interval_secs": 30
    },
    "chat_history": {
      "database_path": "<data_dir>/ai-workshop/chat_history/chat.db",
      "max_conversations_per_instance": 100,
      "retention_days": 90
    },
    "layout": {
      "activitybar_position": "left",
      "sidebar_width": 280,
      "bottom_panel_height": 220,
      "split_ratio": 0.6
    }
  }
}
```

> **注意**：
>
> - 向量检索相关配置已移除，`knowledge.mode` 固定为 `"BM25"`。
> - **布局配置（`layout` 节）为默认真源**：运行时修改写入 localStorage，可通过"保存为默认"写回本文件（见 §5.1）；"重置布局"从本文件重新加载（即使用户曾修改过默认值），另提供"恢复出厂布局"选项。用户亦可通过设置页导出/导入布局文件。
> - **路径占位符**：配置中的 `bm25_index_path`、`skills.base_path`、`chat_history.database_path` 等使用 `<data_dir>` 占位符，运行时由 `config.rs` 基于 **theseus 解析出的数据目录**拼接为真实路径——**不再混用 Tauri `app_data_dir()`**，避免路径分裂；Tauri 不使用 `~/.modrinth`，且硬编码 `~` 在 Windows 上无效。
> - **API Key 安全存储（来自设计评审，2026-08-28 实现修订）**：`providers.*.api_key` 在 `config.json` 中仅保留 `api_key_hint`（如 `****abc`），真实值写入**系统密钥环**（`keyring` crate：macOS Keychain / Windows 凭据管理器 / Linux Secret Service），从首个版本起即加密落盘，跨设备同步时不会暴露明文。实现经 `KeyStore` trait 抽象（`ai_workshop/keystore.rs`）：生产实现 `KeyringKeyStore`，测试/CI 用内存实现；密钥环不可用时**上抛错误**提示用户（绝不回退明文落盘）。`config.rs` 提供 `get_decrypted_api_key(provider_name)` / `set_api_key`，`ProviderFactory` 经 `config_manager` 调用获取真实 Key，前端无需感知加密细节。密钥环尚未落的旧 `secrets.json`（若有）不再读取。
> - **BM25 索引刷新（来自设计评审）**：`KnowledgeRouter` 记录每个 `KnowledgeSource` 的修改时间（mtime）作为第一道过滤，仅当 mtime 变化时才计算内容哈希做二次确认，避免大文档频繁重算；每次检索前检查是否需要重建索引，并提供手动刷新按钮；索引重建作为耗时任务走进度上报机制（见 §6.4），在后台线程进行不阻塞主线程。
> - **日志与监控（来自设计评审）**：集成 `tracing`/`log`，将 AI 工作台内部错误（Provider 调用失败、工具执行异常等）写入文件 `<data_dir>/ai-workshop/logs/app.log`，便于用户报告问题。

### 8.2 核心数据结构

```rust
pub struct ExecutionContext {
    pub instance_id: Option<String>,
    pub theseus_client: Arc<TheseusClient>,
    pub git_client: Arc<GitClient>,
    pub config_manager: Arc<ConfigManager>,
    pub tool_registry: Arc<ToolRegistry>,
    pub toolchain_registry: Arc<ToolchainRegistry>,
    pub knowledge_router: Arc<KnowledgeRouter>,   // 仅 BM25
    pub skill_loader: Arc<SkillLoader>,
    pub instance_lock_manager: Arc<InstanceLockManager>,  // 按实例串行化写操作（见 §7.2）
    pub log_collector: Arc<LogBuffer>,
    pub chat_history: Arc<ChatHistoryRepository>,
    pub cancellation_token: CancellationToken,
    pub tool_timeout_secs: u64,  // 单个工具执行超时（默认 300），超时即取消并报错（见 §6.4）
    pub max_iterations: usize,
    pub token_usage: AtomicUsize,
}
```

---

## 九、测试策略与环境

### 9.1 Mock 测试

在 `mock_enabled = true` 时使用 `MockProvider`，覆盖工具调用、流式响应、错误处理等场景。

### 9.2 单元测试

Rust 侧使用 `#[cfg(test)]`：

- Git 操作（`git_ops.rs`）
- 环形缓冲区（`troubleshooter.rs`）
- 配置解析（`tools/config_ops.rs`）
- 工具链执行（`toolchain/`）
- 安全校验：路径遍历防护、内容净化（`skills/loader.rs`）
- 工具执行的参数校验与错误处理（`tools/`）
- 对话历史数据库操作（`chat_history/repository.rs`）：建表、增删改查、事务
- Provider 适配层（mock 模式下的流式解析等）

前端使用 Vitest 测试 `lib/ai/tools.ts`、`lib/ai/history.ts` 等封装模块。

### 9.3 E2E 测试

- 打开 AI 工作台 → 输入“安装 JEI”→ 确认 → 验证文件系统
- 通过手动工具面板安装 JEI → 验证与 AI 安装结果一致
- 模拟游戏崩溃 → 调用仅在 `cfg(test)` / `debug_assertions` 下启用的独立 Tauri Command `inject_crash_log(log_content)` 向 `LogBuffer` 注入虚假崩溃日志触发排障流程（无需真正启动游戏，且生产环境不可被滥用）→ 验证排障面板弹出、AI 分析、修复建议
- 修改配置 → 验证 Diff 视图展示
- 安全测试：导入恶意技能 → 应被拦截
- 布局调整测试：拖拽面板、切换活动栏位置、重置布局
- 对话持久化测试：发送消息后重启应用 → 历史会话仍在；删除会话 → 数据库记录被清除

> **测试环境准备（来自设计评审）**：E2E 测试前用 `tempfile` crate 创建临时实例目录，填充最小模组与配置以模拟 Modrinth 实例环境，测试后自动清理，避免污染真实数据。

### 9.4 CI/CD

GitHub Actions 流水线：自动运行 Mock 测试 + 单元测试（`cargo nextest`，经 `pnpm run ci` 的 turbo `test` 任务触发；已确认接入）。**2026-08-28 收缩项**：按当前开发阶段决策，E2E 测试（`tauri-driver`/Playwright）、覆盖率 >70% 门禁（llvm-cov）与 `cargo audit` 暂缓接入；前端单测经 Vitest 最小集覆盖 `lib/ai` 封装（见 P3）。后续里程碑再按需补充。

---

## 十、术语表

| 术语 | 英文 | 定义 |
| ------ | ------ | ------ |
| 实例 | Instance | 一个独立的 `.minecraft` 目录，包含特定版本、模组和配置 |
| 原子工具 | Atomic Tool | L1 可执行单元，单一职责，编译时固化，同时供 AI 与 UI 调用 |
| 可执行工具链 | Executable Toolchain | L2 复合可执行流程，由原子工具组合而成 |
| 技能 | Skill | L3 Markdown 知识文档，热加载，用于增强 AI 上下文 |
| 对话持久化 | Chat Persistence | 将对话历史保存至 SQLite，支持恢复、搜索与管理 |
| RAG | Retrieval-Augmented Generation | 检索增强生成（当前仅 BM25） |
| BM25 | BM25 | 关键词检索算法 |
| MCP | Model Context Protocol | AI 模型上下文协议 |
| Function Calling | Function Calling | AI 模型调用预定义函数的机制 |
| 汇合点 | Merge Point | DAG 中多个并行流汇合的位置 |
| Theseus | Theseus | Modrinth App 核心 Rust 库的内部代号 |

---

## 十一、风险与应对预案

| 风险 | 影响 | 缓解措施 | 责任模块 |
| ------ | ------ | ---------- | ---------- |
| AI API 费用激增 / 限流 | 高额账单 | 1. `max_tool_iterations=5` 2. Token 实时显示 3. 写操作须用户确认 | `inference/engine.rs` `StatusBar.vue` |
| 提供商配置缺失 | 用户无法使用 AI | 1. 首次启动且无任何 provider 时直接弹出配置向导对话框（模态），用户可跳过 2. 进入 `AiWorkbench` 时若仍无 provider 则顶部显示醒目配置引导横幅，点击跳转设置页，直至至少一个 provider 配置完成 3. 不预设默认，用户自行选择 4. 设置页提供商选择提供"连接测试"按钮验证 API Key 有效性 | `config.rs` `设置页面` `AiWorkbench.vue` |
| MCP 子进程稳定性 | 进程崩溃 | 1. 默认禁用 2. 健康检查 3. 崩溃自动重启 | `mcp_client.rs` |
| 技能内容安全 | XSS 或路径遍历 | 1. 路径校验 2. 内容净化 3. 默认禁用 | `skills/loader.rs` |
| 上游 Theseus 代码重构 | rebase 冲突 | 每周 rebase，冲突控制在 5 个入侵点 | 全体开发团队 |
| 上下文窗口溢出 | 截断、关键信息丢失 | `context_guard.rs` 动态截断（优先保留对话与工具调用结果）；并引入**压缩/摘要机制**——当接近上限时调用 LLM 对早期检索内容与低优先级上下文生成摘要，压缩后保留摘要而非原文 | `context_guard.rs` |
| 大文件/大规模模组操作导致 UI 冻结 | 交互卡死、用户误以为崩溃 | 所有工具异步执行并**上报进度**（Tauri 事件/WebSocket 推送阶段与百分比），前端以进度条反馈；耗时操作（安装、解压、生成）必须实现进度回调 | `ui_commands.rs` `ToolsView.vue` |
| 并发写操作冲突 | 配置文件/模组列表状态不一致 | 写类工具通过 `InstanceLockManager` 按实例串行化（带超时、禁止嵌套锁），避免 AI 引擎与手动面板并发写同一实例；读操作可并发 | `tools/registry.rs` `ui_commands.rs` |
| 布局跨设备/重装丢失 | 用户需重新调整布局 | 默认布局以 `config.json` 的 `layout` 节为真源，运行时写 localStorage，提供"保存为默认"写回 `config.json`；另支持布局文件导出/导入 | `config.rs` 前端布局组件 |
| 布局系统复杂度高 | 开发周期延长 | 采用成熟库：面板**尺寸拖拽/拆分**用 `vue-resizable-panels`（或 `splitpanes`），活动栏/标签页**顺序拖拽**用 `vue-draggable-plus`（v0.6.1，MIT，基于 SortableJS，仅负责列表排序，不负责面板缩放），并结合 Pinia + `tauri-plugin-window-state` 持久化 | 前端布局组件 |
| SQLite 数据库损坏 | 对话历史丢失 | 1. 启动时执行 `PRAGMA integrity_check` 2. 自动备份（每日轮转） 3. 提供手动备份/恢复功能 | `chat_history/db.rs` |
| 对话历史膨胀 | 占用磁盘、拖慢查询 | 1. 配置 `max_conversations_per_instance` 2. 定期清理超过 `retention_days` 的数据 3. 分页加载消息 | `chat_history/repository.rs` |
| E2E 测试不稳定 | CI 失败 | Mock 模式覆盖大多数场景；真实 API 仅 nightly | `tests/` |

---

**文档状态**：✅ 完整确认（含风险预案，术语已统一为“技能/Skills”，RAG 向量部分暂缓；已据 2026-08-25 联网核查修正入侵点、AI 接入方案、配置存放方式与布局库选型，并据多轮设计评审补充实现细节（取消超时兜底、API Key 解密流程、技能校验失败处理、锁重试、窗口关闭取消、notify 降级、模拟崩溃注入等）；**按产品未上线现状，移除数据库迁移脚本、接口版本化兼容、明文→加密迁移等过度前瞻设计**，可开始开发实施。
**后续维护**：随功能迭代同步更新本文档，每次里程碑后评审风险应对效果。

---

## 十二、联网核查结论与计划修正（2026-08-25）

本章节为基于联网核查（Modrinth `code` 仓库、`crates.io`、`npmjs.com`、Tauri 官方文档）对前文计划所做的**事实修正**，关键改动已同步到 §1.2 / §2.1 / §8.1 / §11。

### 12.1 已核实的上游事实

| 项目 | 核查结果 | 对计划的影响 |
| ------ | ---------- | ---------- |
| Modrinth App 版本 | 最新 release `v0.18.2`（4 天前），仓库 `modrinth/code` 活跃（577 fork） | 分支策略 `upstream/main` 有效 |
| Tauri 版本 | 最新 `2.11.5`；工作区 `tauri` 为 2.x | 计划"Tauri 2.x"一致；注意 2.x 的 `tauri.conf.json` schema 严格 |
| Rust 工具链 | 工作区 `edition = "2024"`、`rust-version = "1.90.0"` | 新增 crate 须兼容 edition 2024 |
| `apps/app` crate 类型 | **二进制 crate**，`package.name = "theseus_gui"`，**无 `lib.rs`** | 入侵点 #2 作废，模块在 `main.rs` 声明 |
| 前端路由 | 文件为 **`apps/app-frontend/src/routes.js`**（`vue-router` + `createWebHistory`），页面经 `pages/index` 注册 | 入侵点 #4 路径修正 |
| 前端主导航 | 由 **`App.vue` 模板内的 `NavButton` 组件**渲染，**无 `components/Nav.vue`** | 入侵点 #5 路径修正 |
| Tauri Commands 注册 | 每个 `api/*.rs` 提供 `init()` 返回带 `invoke_handler(tauri::generate_handler![...])` 的 builder，`main.rs` 以 `.plugin(api::X::init())` 串联 | AI 命令须沿用此插件模式 |
| 已有相关依赖 | `tauri-plugin-window-state`（布局持久化）、`directories`(v6)/`dirs`(v6)（theseus 数据目录解析）、`tauri-plugin-fs`、`tauri-plugin-http` | 可直接复用，减少自研 |
| 前端技术栈 | 已用 `@tanstack/vue-query`、`@modrinth/ui`、`@modrinth/api-client` | AI 前端客户端应与之对齐（如用 `vue-query` 管理异步） |

### 12.2 关键修正清单

1. **入侵点由 6 处修正为 5 处**（§2.1）：删除 `lib.rs` 项；`router.ts` → `routes.js`；`Nav.vue` → `App.vue`/`NavButton`；命令注册改为 `api/ai_workshop.rs` 的 `init()` 插件。
2. **`ai-lib-rust` 定位修正**（§1.2）：它是 "AI-Protocol" 运行时，依赖 GitHub 拉取的 provider manifest，且 `AiClient` 非 `Clone`。**建议改用专用 crate 路线 A**（`async-openai` 覆盖 OpenAI/DeepSeek/Ollama/Custom + `anthropic-sdk-rust` 覆盖 Anthropic），`ai-lib-rust` 降级为可选。计划的 `providers/` 抽象结构保持不变。
3. **`tauri.conf.json` 顶层 `ai_workshop` 无效**（§8.1）：Tauri 2 schema 不允许自定义顶层键，会导致构建失败。改为独立 `ai_workshop.json`（打包进 resources）或数据目录下的 `ai-workshop/config.json`，由 `config.rs` 读写。
4. **数据目录路径修正**（§8.1）：配置中的 `~/.modrinth/...` 不可用（Tauri 不用该路径、Windows 上 `~` 无效）。改为运行时基于 theseus 数据目录（或 `app_data_dir()`）拼接。
5. **布局库修正**（§11）：`vue-draggable-plus` 仅做列表排序（SortableJS），不负责面板缩放。面板尺寸/拆分用 `vue-resizable-panels`（或 `splitpanes`），顺序拖拽才用 `vue-draggable-plus`；持久化结合 `tauri-plugin-window-state`。
6. **缺失依赖需新增**（均不在当前 `apps/app/Cargo.toml`）：`rusqlite`、`tantivy`、`git2`、`scraper`、`html2md`，以及 AI 路线 A 的 `async-openai`、`anthropic-sdk-rust`。前端需新增 `vue-resizable-panels`（可选 `vue-draggable-plus`）。

### 12.3 仍建议保留/注意

- 日志捕获通过 Tauri 侧进程输出重定向、不修改 theseus 库——核查确认 `theseus` 以 `features=["tauri"]` 引入，重定向方案可行。
- `tauri-plugin-window-state` 已存在，可直接用于面板尺寸/位置持久化，与计划"布局状态持久化"目标一致。
- 工作区 Rust 1.90 + edition 2024 较新，引入 `git2`（依赖 `libgit2` 系统库）时需确认 Windows/macOS/Linux CI 的 native 依赖可用；CI 中 Linux 预装 `libgit2-dev`、macOS 用 Homebrew、Windows 用 `vendored` 或 `vcpkg`；若遇阻可评估 `git2` 的 `vendored-libgit2` feature 或改用 `gix`(gitoxide)——但需先评估其 API 完备性（`commit`/`log`/`checkout` 等是否满足需求）。
