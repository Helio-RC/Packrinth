# inference

推理引擎与上下文构建。

## 组成

- `engine.rs`：`InferenceEngine`——单轮与流式多轮循环（`max_tool_iterations` 轮封顶）；写工具调用前经 `ai_confirm_tool` 确认（60s 超时视为拒绝）；工具执行 300s 超时兜底；每轮结束后 `compress_history`（LLM 摘要，`MockProvider` 返回固定摘要）→ `context.trim` 截断兜底。
- `context.rs`：`InferenceContext`——系统提示 + 技能注入（`max_inject_count`）+ 知识检索注入 + 历史载入（分页）。
- 依赖 `context_guard.rs` 的窗口判断。

## 测试

engine 测试覆盖多轮 tool_calls、确认流、压缩/截断、Mock 流式分块。
