use crate::ai_workshop::providers::sse::parse_sse;
use crate::ai_workshop::providers::trait::{
	AiMessage, AiMessageRole, AiProvider, AiResponse, AiUsage, ProviderError, StreamEvent,
	ToolCall, ToolDefinition,
};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const ANTHROPIC_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";

/// Anthropic Claude 提供商。
pub struct AnthropicProvider {
	api_key: String,
	model: String,
}

impl AnthropicProvider {
	pub fn new(api_key: String, model: String) -> Self {
		Self { api_key, model }
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
		let body = build_anthropic_body(&self.model, messages, tools, false);
		let client = reqwest::Client::new();
		let response = client
			.post(ANTHROPIC_ENDPOINT)
			.header("x-api-key", &self.api_key)
			.header("anthropic-version", ANTHROPIC_VERSION)
			.header("content-type", "application/json")
			.json(&body)
			.send()
			.await
			.map_err(|e| ProviderError(format!("请求失败: {e}")))?;
		let status = response.status();
		let text = response
			.text()
			.await
			.map_err(|e| ProviderError(format!("读取响应失败: {e}")))?;
		if !status.is_success() {
			return Err(ProviderError(format!("API 错误 {status}: {text}")));
		}
		let value: serde_json::Value = serde_json::from_str(&text)
			.map_err(|e| ProviderError(format!("解析响应失败: {e}")))?;
		parse_anthropic_response(&value)
	}

	async fn stream(
		&self,
		messages: &[AiMessage],
		tools: &[ToolDefinition],
		tx: tokio::sync::mpsc::Sender<StreamEvent>,
	) -> Result<(), ProviderError> {
		let body = build_anthropic_body(&self.model, messages, tools, true);
		let client = reqwest::Client::new();
		let response = client
			.post(ANTHROPIC_ENDPOINT)
			.header("x-api-key", &self.api_key)
			.header("anthropic-version", ANTHROPIC_VERSION)
			.header("content-type", "application/json")
			.json(&body)
			.send()
			.await
			.map_err(|e| ProviderError(format!("请求失败: {e}")))?;
		let status = response.status();
		if !status.is_success() {
			let text = response
				.text()
				.await
				.map_err(|e| ProviderError(format!("读取响应失败: {e}")))?;
			let error = format!("API 错误 {status}: {text}");
			let _ = tx
				.send(StreamEvent {
					delta: None,
					tool_calls: None,
					usage: None,
					done: true,
					error: Some(error.clone()),
				})
				.await;
			return Err(ProviderError(error));
		}

		let stream = Box::pin(response.bytes_stream());
		let mut tool_acc: Vec<ToolCallAcc> = Vec::new();
		let mut input_tokens: u64 = 0;
		let mut output_tokens: u64 = 0;
		if let Err(e) = parse_sse(stream, |value| {
			let event_type = value.get("type").and_then(|t| t.as_str()).unwrap_or_default();
			match event_type {
				"message_start" => {
					if let Some(message) = value.get("message") {
						if let Some(u) = message.get("usage") {
							input_tokens = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
						}
					}
				}
				"content_block_start" => {
					let index = value.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
					if let Some(block) = value.get("content_block") {
						if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
							while tool_acc.len() <= index {
								tool_acc.push(ToolCallAcc::default());
							}
							let acc = &mut tool_acc[index];
							if let Some(id) = block.get("id").and_then(|i| i.as_str()) {
								acc.id = id.to_string();
							}
							if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
								acc.name = name.to_string();
							}
						}
					}
				}
				"content_block_delta" => {
					if let Some(delta) = value.get("delta") {
						match delta.get("type").and_then(|t| t.as_str()) {
							Some("text_delta") => {
								if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
									if !text.is_empty() {
										let _ = tx.blocking_send(StreamEvent {
											delta: Some(text.to_string()),
											tool_calls: None,
											usage: None,
											done: false,
											error: None,
										});
									}
								}
							}
							Some("input_json_delta") => {
								let index = value
									.get("index")
									.and_then(|i| i.as_u64())
									.unwrap_or(0) as usize;
								if let Some(partial) = delta
									.get("partial_json")
									.and_then(|p| p.as_str())
								{
									while tool_acc.len() <= index {
										tool_acc.push(ToolCallAcc::default());
									}
									tool_acc[index].arguments.push_str(partial);
								}
							}
							_ => {}
						}
					}
				}
				"message_delta" => {
					if let Some(u) = value.get("usage") {
						if let Some(tokens) = u.get("output_tokens").and_then(|v| v.as_u64()) {
							output_tokens = tokens;
						}
					}
				}
				_ => {}
			}
		})
		.await
		{
			let _ = tx
				.send(StreamEvent {
					delta: None,
					tool_calls: None,
					usage: None,
					done: true,
					error: Some(e.to_string()),
				})
				.await;
			return Err(e);
		}

		let tool_calls: Vec<ToolCall> = tool_acc
			.into_iter()
			.enumerate()
			.map(|(index, acc)| ToolCall {
				id: if acc.id.is_empty() {
					format!("toolu_{index}")
				} else {
					acc.id
				},
				name: acc.name,
				arguments: serde_json::from_str(&acc.arguments)
					.unwrap_or_else(|_| serde_json::Value::Object(Default::default())),
			})
			.collect();
		if !tool_calls.is_empty() {
			let _ = tx
				.send(StreamEvent {
					delta: None,
					tool_calls: Some(tool_calls),
					usage: None,
					done: false,
					error: None,
				})
				.await;
		}
		let usage = Some(AiUsage {
			prompt_tokens: input_tokens,
			completion_tokens: output_tokens,
			total_tokens: input_tokens + output_tokens,
		});
		let _ = tx
			.send(StreamEvent {
				delta: None,
				tool_calls: None,
				usage,
				done: true,
				error: None,
			})
			.await;
		Ok(())
	}
}

/// 构造 Anthropic messages 请求体。
fn build_anthropic_body(
	model: &str,
	messages: &[AiMessage],
	tools: &[ToolDefinition],
	stream: bool,
) -> serde_json::Value {
	let system = messages
		.iter()
		.filter(|message| matches!(message.role, AiMessageRole::System))
		.map(|message| message.content.clone())
		.collect::<Vec<_>>()
		.join("\n\n");
	let messages_json: Vec<serde_json::Value> = messages
		.iter()
		.filter_map(anthropic_message_json)
		.collect();
	let tools_json: Vec<serde_json::Value> = tools
		.iter()
		.map(|tool| {
			serde_json::json!({
				"name": tool.name,
				"description": tool.description,
				"input_schema": tool.parameters,
			})
		})
		.collect();
	let mut body = serde_json::json!({
		"model": model,
		"max_tokens": 4096,
		"messages": messages_json,
	});
	if !system.is_empty() {
		body["system"] = serde_json::Value::String(system);
	}
	if !tools_json.is_empty() {
		body["tools"] = serde_json::Value::Array(tools_json);
	}
	if stream {
		body["stream"] = serde_json::Value::Bool(true);
	}
	body
}

/// 将 AiMessage 转换为 Anthropic 消息 JSON（system 消息返回 None，由顶层 system 字段承载）。
fn anthropic_message_json(msg: &AiMessage) -> Option<serde_json::Value> {
	match msg.role {
		AiMessageRole::System => None,
		AiMessageRole::User => {
			if let Some(tool_call_id) = &msg.tool_call_id {
				Some(serde_json::json!({
					"role": "user",
					"content": [{
						"type": "tool_result",
						"tool_use_id": tool_call_id,
						"content": msg.content,
					}]
				}))
			} else {
				Some(serde_json::json!({
					"role": "user",
					"content": [{"type": "text", "text": msg.content}]
				}))
			}
		}
		AiMessageRole::Assistant => {
			let mut content: Vec<serde_json::Value> = Vec::new();
			if !msg.content.is_empty() {
				content.push(serde_json::json!({"type": "text", "text": msg.content}));
			}
			if let Some(tool_calls) = &msg.tool_calls {
				for call in tool_calls {
					content.push(serde_json::json!({
						"type": "tool_use",
						"id": call.id,
						"name": call.name,
						"input": call.arguments,
					}));
				}
			}
			Some(serde_json::json!({
				"role": "assistant",
				"content": content,
			}))
		}
		AiMessageRole::Tool => {
			let tool_call_id = msg.tool_call_id.clone().unwrap_or_default();
			Some(serde_json::json!({
				"role": "user",
				"content": [{
					"type": "tool_result",
					"tool_use_id": tool_call_id,
					"content": msg.content,
				}]
			}))
		}
	}
}

/// 解析 Anthropic 非流式响应。
fn parse_anthropic_response(value: &serde_json::Value) -> Result<AiResponse, ProviderError> {
	let mut content: Option<String> = None;
	let mut tool_calls = Vec::new();
	if let Some(blocks) = value.get("content").and_then(|c| c.as_array()) {
		for block in blocks {
			match block.get("type").and_then(|t| t.as_str()) {
				Some("text") => {
					if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
						content = Some(match content {
							Some(existing) => format!("{existing}{text}"),
							None => text.to_string(),
						});
					}
				}
				Some("tool_use") => {
					let id = block
						.get("id")
						.and_then(|i| i.as_str())
						.unwrap_or_default()
						.to_string();
					let name = block
						.get("name")
						.and_then(|n| n.as_str())
						.unwrap_or_default()
						.to_string();
					let arguments = block
						.get("input")
						.cloned()
						.unwrap_or_else(|| serde_json::Value::Object(Default::default()));
					tool_calls.push(ToolCall { id, name, arguments });
				}
				_ => {}
			}
		}
	}
	let usage = value.get("usage").and_then(|u| {
		let prompt_tokens = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
		let completion_tokens = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
		Some(AiUsage {
			prompt_tokens,
			completion_tokens,
			total_tokens: prompt_tokens + completion_tokens,
		})
	});
	Ok(AiResponse { content, tool_calls, usage })
}

/// 按 index 累积的流式工具调用片段。
#[derive(Default)]
struct ToolCallAcc {
	id: String,
	name: String,
	arguments: String,
}
