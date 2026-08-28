#![allow(deprecated)]
// === AI-WORKSHOP START ===
// OpenAI 兼容提供商（async-openai）：服务 openai / deepseek / custom 端点。
// Ollama 亦走此实现（无鉴权）。
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatChoice, ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
    ChatCompletionRequestAssistantMessage,
    ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent,
    ChatCompletionRequestToolMessage, ChatCompletionRequestToolMessageContent,
    ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
    ChatCompletionTool, ChatCompletionTools, CreateChatCompletionRequest,
    CreateChatCompletionRequestArgs, CreateChatCompletionResponse,
    FunctionCall, FunctionObject,
};
use futures_util::StreamExt;

use crate::ai_workshop::providers::provider_trait::{
    AiMessage, AiMessageRole, AiProvider, AiResponse, AiUsage, ProviderError,
    StreamEvent, ToolCall, ToolDefinition,
};

/// OpenAI 兼容提供商（同时服务 openai / deepseek / custom，base_url 不同）。
pub struct OpenAIProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAIProvider {
    pub fn new(
        api_key: String,
        model: String,
        base_url: Option<String>,
    ) -> Self {
        Self {
            api_key,
            model,
            base_url: base_url
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        }
    }

    fn client(&self) -> Client<OpenAIConfig> {
        let config = OpenAIConfig::new()
            .with_api_key(self.api_key.clone())
            .with_api_base(self.base_url.clone());
        Client::with_config(config)
    }
}

#[async_trait::async_trait]
#[allow(deprecated)]
impl AiProvider for OpenAIProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    async fn chat(
        &self,
        messages: &[AiMessage],
        tools: &[ToolDefinition],
    ) -> Result<AiResponse, ProviderError> {
        let request = build_request(&self.model, messages, tools, false);
        let response = self
            .client()
            .chat()
            .create(request)
            .await
            .map_err(as_provider_err)?;
        Ok(response_to_ai(response))
    }

    async fn stream(
        &self,
        messages: &[AiMessage],
        tools: &[ToolDefinition],
        tx: tokio::sync::mpsc::Sender<StreamEvent>,
    ) -> Result<(), ProviderError> {
        let request = build_request(&self.model, messages, tools, true);
        let mut stream = self
            .client()
            .chat()
            .create_stream(request)
            .await
            .map_err(as_provider_err)?;

        let send = |event: StreamEvent| {
            let _ = tx.blocking_send(event);
        };

        let mut tool_acc: Vec<ToolCallAcc> = Vec::new();
        let mut usage: Option<AiUsage> = None;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(as_provider_err)?;
            if let Some(u) = chunk.usage {
                usage = Some(AiUsage {
                    prompt_tokens: u.prompt_tokens as u64,
                    completion_tokens: u.completion_tokens as u64,
                    total_tokens: u.total_tokens as u64,
                });
            }
            let Some(choice) = chunk.choices.first() else {
                continue;
            };
            if let Some(content) = &choice.delta.content
                && !content.is_empty()
            {
                send(StreamEvent {
                    delta: Some(content.clone()),
                    tool_calls: None,
                    usage: None,
                    done: false,
                    error: None,
                });
            }
            if let Some(calls) = &choice.delta.tool_calls {
                for call in calls {
                    let index = call.index as usize;
                    while tool_acc.len() <= index {
                        tool_acc.push(ToolCallAcc::default());
                    }
                    let acc = &mut tool_acc[index];
                    if let Some(id) = &call.id {
                        acc.id = id.clone();
                    }
                    if let Some(function) = &call.function {
                        if let Some(name) = &function.name {
                            acc.name = name.clone();
                        }
                        if let Some(args) = &function.arguments {
                            acc.arguments.push_str(args);
                        }
                    }
                }
            }
        }

        let tool_calls: Vec<ToolCall> = tool_acc
            .into_iter()
            .enumerate()
            .map(|(index, acc)| ToolCall {
                id: if acc.id.is_empty() {
                    format!("call_{index}")
                } else {
                    acc.id
                },
                name: acc.name,
                arguments: serde_json::from_str(&acc.arguments).unwrap_or_else(
                    |_| serde_json::Value::Object(serde_json::Map::default()),
                ),
            })
            .collect();
        if !tool_calls.is_empty() {
            send(StreamEvent {
                delta: None,
                tool_calls: Some(tool_calls),
                usage: None,
                done: false,
                error: None,
            });
        }
        send(StreamEvent {
            delta: None,
            tool_calls: None,
            usage,
            done: true,
            error: None,
        });
        Ok(())
    }
}

fn as_provider_err(e: impl std::fmt::Display) -> ProviderError {
    ProviderError(e.to_string())
}

/// 构造 OpenAI 兼容请求（消息 + 工具定义，含角色映射与多 system 合并）。
fn build_request(
    model: &str,
    messages: &[AiMessage],
    tools: &[ToolDefinition],
    stream: bool,
) -> CreateChatCompletionRequest {
    let mut system = String::new();
    let mut request_messages = Vec::new();
    for message in messages {
        match message.role {
            AiMessageRole::System => {
                if !system.is_empty() {
                    system.push('\n');
                }
                system.push_str(&message.content);
            }
            AiMessageRole::User => {
                request_messages.push(ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessage {
                        content: ChatCompletionRequestUserMessageContent::Text(
                            message.content.clone(),
                        ),
                        name: None,
                    },
                ))
            }
            AiMessageRole::Assistant => {
                let tool_calls = message.tool_calls.as_ref().map(|calls| {
                    calls
                        .iter()
                        .map(|call| {
                            ChatCompletionMessageToolCalls::Function(
                                ChatCompletionMessageToolCall {
                                    id: call.id.clone(),
                                    function: FunctionCall {
                                        name: call.name.clone(),
                                        arguments: serde_json::to_string(
                                            &call.arguments,
                                        )
                                        .unwrap_or_else(|_| "{}".to_string()),
                                    },
                                },
                            )
                        })
                        .collect()
                });
                request_messages.push(ChatCompletionRequestMessage::Assistant(
                    ChatCompletionRequestAssistantMessage {
                        content: Some(
                            ChatCompletionRequestAssistantMessageContent::Text(
                                message.content.clone(),
                            ),
                        ),
                        name: None,
                        refusal: None,
                        audio: None,
                        tool_calls,
                        function_call: None,
                    },
                ));
            }
            AiMessageRole::Tool => {
                request_messages.push(ChatCompletionRequestMessage::Tool(
                    ChatCompletionRequestToolMessage {
                        content: ChatCompletionRequestToolMessageContent::Text(
                            message.content.clone(),
                        ),
                        tool_call_id: message
                            .tool_call_id
                            .clone()
                            .unwrap_or_default(),
                    },
                ));
            }
        }
    }

    // System 内容合并后以 system 角色消息置顶（兼容所有 OpenAI 兼容端点）。
    if !system.is_empty() {
        request_messages.insert(
            0,
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessage {
                    content: ChatCompletionRequestSystemMessageContent::Text(
                        system,
                    ),
                    name: None,
                },
            ),
        );
    }

    let mut builder = CreateChatCompletionRequestArgs::default();
    builder
        .model(model.to_string())
        .messages(request_messages)
        .stream(stream);
    if !tools.is_empty() {
        builder.tools(tools.iter().map(to_openai_tool).collect::<Vec<_>>());
    }
    builder.build().expect("valid chat completion request")
}

/// 工具定义 → OpenAI 格式（含单层嵌套 JSON Schema 参数）。
fn to_openai_tool(tool: &ToolDefinition) -> ChatCompletionTools {
    ChatCompletionTools::Function(ChatCompletionTool {
        function: FunctionObject {
            name: tool.name.clone(),
            description: Some(tool.description.clone()),
            parameters: Some(tool.parameters.clone()),
            strict: None,
        },
    })
}

fn response_to_ai(response: CreateChatCompletionResponse) -> AiResponse {
    let choice: Option<ChatChoice> = response.choices.into_iter().next();
    let usage = response.usage.map(|u| AiUsage {
        prompt_tokens: u.prompt_tokens as u64,
        completion_tokens: u.completion_tokens as u64,
        total_tokens: u.total_tokens as u64,
    });
    match choice {
        Some(choice) => {
            let message = choice.message;
            let tool_calls = message
                .tool_calls
                .map(|calls| {
                    calls
                        .into_iter()
                        .filter_map(|call| match call {
                            ChatCompletionMessageToolCalls::Function(call) => {
                                Some(ToolCall {
                                    id: call.id,
                                    name: call.function.name,
                                    arguments: serde_json::from_str(
                                        &call.function.arguments,
                                    )
                                    .unwrap_or_else(|_| {
                                        serde_json::Value::Object(
                                            serde_json::Map::default(),
                                        )
                                    }),
                                })
                            }
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            AiResponse {
                content: message.content,
                tool_calls,
                usage,
            }
        }
        None => AiResponse {
            content: None,
            tool_calls: Vec::new(),
            usage,
        },
    }
}

/// 按 index 累积的流式工具调用片段。
#[derive(Default)]
struct ToolCallAcc {
    id: String,
    name: String,
    arguments: String,
}
// === AI-WORKSHOP END ===
