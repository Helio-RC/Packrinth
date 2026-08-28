// === AI-WORKSHOP START ===
// Anthropic Claude 提供商（anthropic-sdk-rust）。
use anthropic_sdk::types::tools::ToolInputSchema;
use anthropic_sdk::{
    Anthropic, ContentBlock, ContentBlockDelta, ContentBlockParam, Message,
    MessageContent, MessageCreateParams, MessageParam, MessageStreamEvent,
    Role, Tool,
};
use futures_util::StreamExt;

use crate::ai_workshop::providers::provider_trait::{
    AiMessage, AiMessageRole, AiProvider, AiResponse, AiUsage, ProviderError,
    StreamEvent, ToolCall, ToolDefinition,
};

const ANTHROPIC_MAX_TOKENS: u32 = 4096;

/// Anthropic Claude 提供商。
pub struct AnthropicProvider {
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }

    fn client(&self) -> Result<Anthropic, ProviderError> {
        Anthropic::new(self.api_key.clone())
            .map_err(|e| ProviderError(e.to_string()))
    }
}

#[async_trait::async_trait]
impl AiProvider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    async fn chat(
        &self,
        messages: &[AiMessage],
        tools: &[ToolDefinition],
    ) -> Result<AiResponse, ProviderError> {
        let message = self
            .client()?
            .messages()
            .create(build_params(&self.model, messages, tools))
            .await
            .map_err(|e| ProviderError(e.to_string()))?;
        Ok(message_to_ai(message))
    }

    async fn stream(
        &self,
        messages: &[AiMessage],
        tools: &[ToolDefinition],
        tx: tokio::sync::mpsc::Sender<StreamEvent>,
    ) -> Result<(), ProviderError> {
        let mut stream = self
            .client()?
            .messages()
            .create_stream(build_params(&self.model, messages, tools))
            .await
            .map_err(|e| ProviderError(e.to_string()))?;

        let send = |event: StreamEvent| {
            let _ = tx.blocking_send(event);
        };
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ProviderError(e.to_string()))?;
            let event = match chunk {
                MessageStreamEvent::ContentBlockDelta {
                    delta: ContentBlockDelta::TextDelta { text },
                    ..
                } => StreamEvent {
                    delta: Some(text),
                    tool_calls: None,
                    usage: None,
                    done: false,
                    error: None,
                },
                MessageStreamEvent::MessageDelta { usage, .. } => StreamEvent {
                    delta: None,
                    tool_calls: None,
                    usage: Some(AiUsage {
                        prompt_tokens: usage.input_tokens.unwrap_or(0) as u64,
                        completion_tokens: usage.output_tokens as u64,
                        total_tokens: usage.input_tokens.unwrap_or(0) as u64
                            + usage.output_tokens as u64,
                    }),
                    done: false,
                    error: None,
                },
                _ => continue,
            };
            send(event);
        }

        // 结束事件后从累积快照提取最终工具调用与用量（final_message 返回完整 Message）。
        let final_message = stream
            .final_message()
            .await
            .map_err(|e| ProviderError(e.to_string()))?;
        let ai = message_to_ai(final_message.clone());
        if !ai.tool_calls.is_empty() {
            send(StreamEvent {
                delta: None,
                tool_calls: Some(ai.tool_calls),
                usage: None,
                done: false,
                error: None,
            });
        }
        send(StreamEvent {
            delta: None,
            tool_calls: None,
            usage: ai.usage,
            done: true,
            error: None,
        });
        Ok(())
    }
}

/// 构造 Anthropic 请求参数：多 System 合并为 system 字段；工具消息映射为 tool_result 用户消息。
fn build_params(
    model: &str,
    messages: &[AiMessage],
    tools: &[ToolDefinition],
) -> MessageCreateParams {
    let mut system = String::new();
    let mut params_messages = Vec::new();
    for message in messages {
        match message.role {
            AiMessageRole::System => {
                if !system.is_empty() {
                    system.push('\n');
                }
                system.push_str(&message.content);
            }
            AiMessageRole::User => params_messages.push(MessageParam {
                role: Role::User,
                content: MessageContent::Text(message.content.clone()),
            }),
            AiMessageRole::Assistant => {
                params_messages.push(MessageParam {
                    role: Role::Assistant,
                    content: MessageContent::Text(message.content.clone()),
                });
                if let Some(calls) = &message.tool_calls {
                    let blocks = calls
                        .iter()
                        .map(|call| ContentBlockParam::ToolUse {
                            id: call.id.clone(),
                            name: call.name.clone(),
                            input: call.arguments.clone(),
                        })
                        .collect();
                    params_messages.push(MessageParam {
                        role: Role::Assistant,
                        content: MessageContent::Blocks(blocks),
                    });
                }
            }
            AiMessageRole::Tool => {
                params_messages.push(MessageParam {
                    role: Role::User,
                    content: MessageContent::Blocks(vec![
                        ContentBlockParam::ToolResult {
                            tool_use_id: message
                                .tool_call_id
                                .clone()
                                .unwrap_or_default(),
                            content: Some(message.content.clone()),
                            is_error: None,
                        },
                    ]),
                });
            }
        }
    }

    MessageCreateParams {
        model: model.to_string(),
        max_tokens: ANTHROPIC_MAX_TOKENS,
        messages: params_messages,
        system: (!system.is_empty()).then_some(system),
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: None,
        stream: None,
        tools: (!tools.is_empty())
            .then(|| tools.iter().map(to_anthropic_tool).collect()),
        tool_choice: None,
        metadata: None,
    }
}

/// 工具定义 → Anthropic 格式；参数 JSON Schema 展开为 properties + required。
fn to_anthropic_tool(tool: &ToolDefinition) -> Tool {
    let schema = &tool.parameters;
    let (props, required) = match schema.get("properties") {
        Some(serde_json::Value::Object(map)) => {
            let required = schema
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            (map.clone(), required)
        }
        _ => (serde_json::Map::new(), Vec::new()),
    };
    // 其余 schema 键（title/description 等）放入 additional 展平字段。
    let mut additional = serde_json::Map::new();
    if let Some(obj) = schema.as_object() {
        for (key, value) in obj.iter() {
            if !matches!(key.as_str(), "type" | "properties" | "required") {
                additional.insert(key.clone(), value.clone());
            }
        }
    }
    Tool {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: props,
            required,
            additional,
        },
    }
}

/// Anthropic Message → 统一 AiResponse（文本块拼接 + 工具调用 + 用量）。
fn message_to_ai(message: Message) -> AiResponse {
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    for block in &message.content {
        match block {
            ContentBlock::Text { text } => content.push_str(text),
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: input.clone(),
                })
            }
            _ => {}
        }
    }
    let usage = Some(AiUsage {
        prompt_tokens: message.usage.input_tokens as u64,
        completion_tokens: message.usage.output_tokens as u64,
        total_tokens: (message.usage.input_tokens as u64)
            + (message.usage.output_tokens as u64),
    });
    AiResponse {
        content: (!content.trim().is_empty()).then_some(content),
        tool_calls,
        usage,
    }
}

// === AI-WORKSHOP END ===
