# tools

L1 原子工具（AI 引擎与手动面板共用，`ToolRegistry`）。

## 组成

- `registry.rs`：`Tool` trait + `ToolInfo`（含 JSON Schema）/ `ToolDomain`（含 `Mcp`）/ `remove`（MCP 热刷新）。
- `context.rs`：`ExecutionContext`（取消令牌/进度回调/实例写锁管理器）；`InstanceLockManager`（按实例互斥，30s 超时提示"另一个操作正在修改此实例"）；`TaskRegistry`（task_id → 取消令牌，窗口关闭 cancel_all）；`ProgressPayload` 结构。
- `mod_ops.rs`：search/get/install/remove/update/list_installed/resolve_dependencies/get_instance_info/create/duplicate/delete/launch。
- `config_ops.rs`：read/write/rollback/list/diff_config（含备份）。
- `script_gen.rs`：generate_kubejs_script / generate_crafttweaker_script。
- `knowledge_ops.rs`：crawl_document（白名单+分块）。
- `git_ops.rs`（git2）：init/status/log/commit/checkout/branch/diff。

## 测试

参数校验、错误路径、Schema 断言、写锁并发/超时、工具超时取消均覆盖（详见各文件尾部 tests）。
