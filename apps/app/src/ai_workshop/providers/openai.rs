use crate::ai_workshop::providers::sse::parse_sse;
use crate::ai_workshop::providers::trait::{
	AiMessage, AiMessageRole, AiProvider, AiResponse, AiUsage, ProviderError, StreamEvent,
	ToolCall, ToolDefinition,
};

/// OpenAI 兼容提供商（同时服务 openai / deepseek / custom，base_url 不同）。
pub struct OpenAIProvider {
	api_key: String,
	model: String,
	base_url: String,
}

impl OpenAIProvider {
	pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
		Self {
			api_key,
			model,
			base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
		}
	}
}

#[async_trait::async_trait]
impl AiProvider for OpenAIProvider {
	fn name(&self) -> &'static str {
		"openai"
	}

	async fn chat(
		&self,
		messages: &[AiMessage],
		tools: &[ToolDefinition],
	) -> Result<AiResponse, ProviderError> {
		let client = reqwest::Client::new();
		let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
		let body = build_chat_body(&self.model, messages, tools, false);
		chat_openai_compatible(&client, &url, Some(&self.api_key), body).await
	}

	async fn stream(
		&self,
		messages: &[AiMessage],
		tools: &[ToolDefinition],
		tx: tokio::sync::mpsc::Sender<StreamEvent>,
	) -> Result<(), ProviderError> {
		let client = reqwest::Client::new();
		let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
		let body = build_chat_body(&self.model, messages, tools, true);
		stream_openai_compatible(&client, &url, Some(&self.api_key), body, &tx).await
	}
}

/// 构造 OpenAI 兼容的 chat/completions 请求体。
pub(crate) fn build_chat_body(
	model: &str,
	messages: &[AiMessage],
	tools: &[ToolDefinition],
	stream: bool,
) -> serde_json::Value {
	let messages_json: Vec<serde_json::Value> = messages.iter().map(openai_message_json).collect();
	let tools_json: Vec<serde_json::Value> = tools
		.iter()
		.map(|tool| {
			serde_json::json!({
				"type": "function",
				"function": {
					"name": tool.name,
					"description": tool.description,
					"parameters": tool.parameters,
				}
			})
		})
		.collect();
	let mut body = serde_json::json!({
		"model": model,
		"messages": messages_json,
	});
	if !tools_json.is_empty() {
		body["tools"] = serde_json::Value::Array(tools_json);
		body["tool_choice"] = serde_json::Value::String("auto".to_string());
	}
	if stream {
		body["stream"] = serde_json::Value::Bool(true);
	}
	body
}

/// 将 AiMessage 转换为 OpenAI 消息 JSON。
pub(crate) fn openai_message_json(msg: &AiMessage) -> serde_json::Value {
	let mut obj = serde_json::Map::new();
	obj.insert(
		"role".to_string(),
		serde_json::Value::String(msg.role.as_str().to_string()),
	);
	match msg.role {
		AiMessageRole::Tool => {
			obj.insert("content".to_string(), serde_json::Value::String(msg.content.clone()));
			if let Some(tool_call_id) = &msg.tool_call_id {
				obj.insert(
					"tool_call_id".to_string(),
					serde_json::Value::String(tool_call_id.clone()),
				);
			}
		}
		AiMessageRole::Assistant => {
			obj.insert("content".to_string(), serde_json::Value::String(msg.content.clone()));
			if let Some(tool_calls) = &msg.tool_calls {
				let tool_calls_json: Vec<serde_json::Value> = tool_calls
					.iter()
					.map(|call| {
						serde_json::json!({
							"id": call.id,
							"type": "function",
							"function": {
								"name": call.name,
								"arguments": serde_json::to_string(&call.arguments).unwrap_or_default(),
							}
						})
					})
					.collect();
				obj.insert("tool_calls".to_string(), serde_json::Value::Array(tool_calls_json));
			}
		}
		_ => {
			obj.insert("content".to_string(), serde_json::Value::String(msg.content.clone()));
		}
	}
	serde_json::Value::Object(obj)
}

/// 解析 OpenAI 兼容的非流式响应。
pub(crate) fn parse_chat_response(value: &serde_json::Value) -> Result<AiResponse, ProviderError> {
	let choice = value
		.get("choices")
		.and_then(|choices| choices.as_array())
		.and_then(|choices| choices.first())
		.ok_or_else(|| ProviderError("响应缺少 choices".to_string()))?;
	let message = choice
		.get("message")
		.cloned()
		.unwrap_or_else(|| serde_json::Value::Null);
	let content = message
		.get("content")
		.and_then(|content| content.as_str())
		.map(|s| s.to_string());
	let mut tool_calls = Vec::new();
	if let Some(calls) = message.get("tool_calls").and_then(|calls| calls.as_array()) {
		for call in calls {
			let id = call
				.get("id")
				.and_then(|id| id.as_str())
				.unwrap_or_default()
				.to_string();
			let name = call
				.get("function")
				.and_then(|function| function.get("name"))
				.and_then(|name| name.as_str())
				.unwrap_or_default()
				.to_string();
			let arguments_str = call
				.get("function")
				.and_then(|function| function.get("arguments"))
				.and_then(|arguments| arguments.as_str())
				.unwrap_or("{}");
			let arguments = serde_json::from_str(arguments_str)
				.unwrap_or_else(|_| serde_json::Value::Object(Default::default()));
			tool_calls.push(ToolCall { id, name, arguments });
		}
	}
	let usage = value.get("usage").and_then(parse_usage);
	Ok(AiResponse { content, tool_calls, usage })
}

/// 解析 OpenAI 兼容的 usage 字段。
pub(crate) fn parse_usage(value: &serde_json::Value) -> Option<AiUsage> {
	let prompt_tokens = value.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
	let completion_tokens = value
		.get("completion_tokens")
		.and_then(|v| v.as_u64())
		.unwrap_or(0);
	let total_tokens = value
		.get("total_tokens")
		.and_then(|v| v.as_u64())
		.unwrap_or(prompt_tokens + completion_tokens);
	Some(AiUsage {
		prompt_tokens,
		completion_tokens,
		total_tokens,
	})
}

/// 发送 OpenAI 兼容的非流式请求并解析响应。
pub(crate) async fn chat_openai_compatible(
	client: &reqwest::Client,
	url: &str,
	api_key: Option<&str>,
	body: serde_json::Value,
) -> Result<AiResponse, ProviderError> {
	let mut request = client.post(url).json(&body);
	if let Some(key) = api_key {
		request = request.bearer_auth(key);
	}
	let response = request
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
	parse_chat_response(&value)
}

/// 发送 OpenAI 兼容的流式请求，解析 SSE 并逐段发送 StreamEvent。
pub(crate) async fn stream_openai_compatible(
	client: &reqwest::Client,
	url: &str,
	api_key: Option<&str>,
	body: serde_json::Value,
	tx: &tokio::sync::mpsc::Sender<StreamEvent>,
) -> Result<(), ProviderError> {
	let mut request = client.post(url).json(&body);
	if let Some(key) = api_key {
		request = request.bearer_auth(key);
	}
	let response = request
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
	let mut usage: Option<AiUsage> = None;
	if let Err(e) = parse_sse(stream, |value| {
		if let Some(u) = value.get("usage") {
			usage = parse_usage(u);
		}
		if let Some(choices) = value.get("choices").and_then(|c| c.as_array()) {
			if let Some(choice) = choices.first() {
				if let Some(delta) = choice.get("delta") {
					if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
						if !content.is_empty() {
							let _ = tx.blocking_send(StreamEvent {
								delta: Some(content.to_string()),
								tool_calls: None,
								usage: None,
								done: false,
								error: None,
							});
						}
					}
					if let Some(calls) = delta.get("tool_calls").and_then(|c| c.as_array()) {
						for call in calls {
							let index = call
								.get("index")
								.and_then(|i| i.as_u64())
								.unwrap_or(0) as usize;
							while tool_acc.len() <= index {
								tool_acc.push(ToolCallAcc::default());
							}
							let acc = &mut tool_acc[index];
							if let Some(id) = call.get("id").and_then(|i| i.as_str()) {
								acc.id = id.to_string();
							}
							if let Some(name) = call
								.get("function")
								.and_then(|f| f.get("name"))
								.and_then(|n| n.as_str())
							{
								acc.name = name.to_string();
							}
							if let Some(args) = call
								.get("function")
								.and_then(|f| f.get("arguments"))
								.and_then(|a| a.as_str())
							{
								acc.arguments.push_str(args);
							}
						}
					}
				}
			}
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
				format!("call_{index}")
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

/// 按 index 累积的流式工具调用片段。
#[derive(Default)]
struct ToolCallAcc {
	id: String,
	name: String,
	arguments: String,
}
