# providers

AI 提供商适配层。

## 组成

- `provider_trait.rs`：`AiProvider{ chat, stream, summarize }` + 消息/工具/用量/流事件类型。
- `factory.rs`：按配置创建提供商（mock 优先；真实 Key 经 `ConfigManager::get_decrypted_api_key` 从密钥环获取）。
- `openai.rs`：`async-openai` 实现（OpenAI / DeepSeek / Custom 端点；流式按 index 累积工具调用参数后整发）。
- `anthropic.rs`：`anthropic-sdk-rust` 实现（tools 走 `input_schema`，tool_result 走用户消息块；流式经 MessageStream）。
- `ollama.rs`：无鉴权复用 OpenAIProvider（自动补 `/v1`）。
- `mock.rs`：`MockProvider`（45 条触发规则 + 流式分块 + 固定摘要）。自研 `sse.rs` 已删除（改用 crate）。

## 测试

mock 覆盖案例匹配、流式分块、错误注入；provider_trait 无独立测试（经 engine mock 链路覆盖）。
