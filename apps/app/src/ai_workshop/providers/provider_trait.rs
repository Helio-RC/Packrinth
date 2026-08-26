use serde::Serialize;

/// 工具定义：名称、描述与 JSON Schema 参数。
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct ToolDefinition {
	pub name: String,
	pub description: String,
	pub parameters: serde_json::Value,
}

/// 消息角色。
#[derive(Clone, Debug)]
pub enum AiMessageRole {
	System,
	User,
	Assistant,
	Tool,
}

impl AiMessageRole {
	pub fn as_str(&self) -> &'static str {
		match self {
			AiMessageRole::System => "system",
			AiMessageRole::User => "user",
			AiMessageRole::Assistant => "assistant",
			AiMessageRole::Tool => "tool",
		}
	}
}

/// 与提供商无关的对话消息。
#[derive(Clone, Debug)]
pub struct AiMessage {
	pub role: AiMessageRole,
	pub content: String,
	pub tool_calls: Option<Vec<ToolCall>>,
	pub tool_call_id: Option<String>,
	pub name: Option<String>,
}

impl AiMessage {
	pub fn system(content: String) -> Self {
		Self {
			role: AiMessageRole::System,
			content,
			tool_calls: None,
			tool_call_id: None,
			name: None,
		}
	}

	pub fn user(content: String) -> Self {
		Self {
			role: AiMessageRole::User,
			content,
			tool_calls: None,
			tool_call_id: None,
			name: None,
		}
	}

	pub fn assistant(content: String) -> Self {
		Self {
			role: AiMessageRole::Assistant,
			content,
			tool_calls: None,
			tool_call_id: None,
			name: None,
		}
	}

	pub fn tool_result(tool_call_id: String, content: String) -> Self {
		Self {
			role: AiMessageRole::Tool,
			content,
			tool_calls: None,
			tool_call_id: Some(tool_call_id),
			name: None,
		}
	}
}

/// 工具调用。
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct ToolCall {
	pub id: String,
	pub name: String,
	pub arguments: serde_json::Value,
}

/// Token 用量统计。
#[derive(Clone, Debug, Serialize)]
pub struct AiUsage {
	pub prompt_tokens: u64,
	pub completion_tokens: u64,
	pub total_tokens: u64,
}

/// 非流式响应。
#[derive(Clone, Debug, Serialize)]
pub struct AiResponse {
	pub content: Option<String>,
	pub tool_calls: Vec<ToolCall>,
	pub usage: Option<AiUsage>,
}

/// 流式事件。
#[derive(Clone, Debug, Serialize)]
pub struct StreamEvent {
	pub delta: Option<String>,
	pub tool_calls: Option<Vec<ToolCall>>,
	pub usage: Option<AiUsage>,
	pub done: bool,
	pub error: Option<String>,
}

/// 提供商错误。
#[derive(Clone, Debug)]
pub struct ProviderError(pub String);

impl std::fmt::Display for ProviderError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl std::error::Error for ProviderError {}

impl From<String> for ProviderError {
	fn from(s: String) -> Self {
		ProviderError(s)
	}
}

/// AI 提供商抽象：非流式 chat 与流式 stream。
#[async_trait::async_trait]
pub trait AiProvider: Send + Sync {
	fn name(&self) -> &'static str;
	async fn chat(
		&self,
		messages: &[AiMessage],
		tools: &[ToolDefinition],
	) -> Result<AiResponse, ProviderError>;
	async fn stream(
		&self,
		messages: &[AiMessage],
		tools: &[ToolDefinition],
		tx: tokio::sync::mpsc::Sender<StreamEvent>,
	) -> Result<(), ProviderError>;
}
