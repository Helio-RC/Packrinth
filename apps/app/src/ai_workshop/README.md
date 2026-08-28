# ai_workshop

AI 工作台 Rust 后端模块包（Tauri 插件 `ai_workshop`，由 `api/ai_workshop.rs` 的 `init()` 注册）。

## 模块职责

| 模块 | 职责 |
| --- | --- |
| `config.rs` | 配置读写（`<data_dir>/ai-workshop/config.json`，snake_case 文件格式；IPC 边界 camelCase DTO）；API Key 经 `keystore` 访问 |
| `keystore.rs` | `KeyStore` trait：生产 `KeyringKeyStore`（系统密钥环）/ 测试内存实现；密钥环失败上抛，不回退明文 |
| `providers/` | AI 提供商适配：`AiProvider` trait（chat/stream/summarize）；async-openai（OpenAI/DeepSeek/Custom/Ollama 兼容）+ anthropic-sdk-rust（Claude）+ MockProvider |
| `inference/` | `InferenceEngine` 多轮 tool_calls 循环（最大 `max_tool_iterations` 轮）；`InferenceContext` 构建系统提示/注入知识技能；上下文 LLM 压缩（`context_guard` 触发） |
| `chat_history/` | SQLite 对话持久化（`rusqlite` bundled）；会话/消息 CRUD + 分页（默认 50 条） |
| `tools/` | L1 原子工具：`ToolRegistry` + `Tool` trait；`ExecutionContext`（取消令牌/进度/实例写锁）；mod/config/script/knowledge/git 工具 |
| `toolchain/` | L2 可执行工具链：`ExecutableToolchain` trait + 注册表；内置 kubejs_gen / ct_gen / ftb_recipe / mod_config / export_mods |
| `skills/` | L3 技能：扫描解析 `skill.toml`+`guide.md`（校验规则 §7.4）、三层净化 `sanitizer.rs`、关键词匹配、notify 热加载 |
| `knowledge/` | BM25（tantivy）知识检索：source/router/chunker/crawler（域名白名单 + html2md） |
| `mcp_client.rs` | MCP stdio 客户端：initialize → tools/list 注册 → ping 健康检查 → 崩溃自动重启 |
| `troubleshooter.rs` | 日志环形缓冲区（容量可配、行数阈值/定时落盘）+ `inject_crash_log`（仅测试） |
| `context_guard.rs` | 上下文窗口看门狗：`summarize_needed` / `enforce_window`（截断兜底） |
| `git_ops.rs` | Git 原子工具（git2）：init/status/log/commit/checkout/branch/diff |
| `ui_commands.rs` | 工具/工具链执行的 Tauri 命令层（进度、取消、300s 超时兜底） |

## 测试策略

- 各模块自带 `#[cfg(test)]`（153 项覆盖配置/密钥、引擎多轮、持久化分页、技能净化与校验、BM25 mtime 增量、Git、环形缓冲区、锁/取消、MCP 响应解析）。
- 运行：`cargo test -p theseus_gui ai_workshop`（CI 经 turbo `test` 任务执行 `cargo nextest run`）。
