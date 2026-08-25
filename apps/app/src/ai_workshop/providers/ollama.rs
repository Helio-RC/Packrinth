use crate::ai_workshop::providers::openai::{
	build_chat_body, chat_openai_compatible, stream_openai_compatible,
};
use crate::ai_workshop::providers::trait::{
	AiMessage, AiProvider, AiResponse, ProviderError, StreamEvent, ToolDefinition,
};

/// Ollama 本地模型提供商（兼容 OpenAI 端点）。
pub struct OllamaProvider {
	base_url: String,
	model: String,
}

impl OllamaProvider {
	pub fn new(model: String, base_url: Option<String>) -> Self {
		Self {
			base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
			model,
		}
	}
}

#[async_trait::async_trait]
impl AiProvider for OllamaProvider {
	fn name(&self) -> &'static str {
		"ollama"
	}

	async fn chat(
		&self,
		messages: &[AiMessage],
		tools: &[ToolDefinition],
	) -> Result<AiResponse, ProviderError> {
		let client = reqwest::Client::new();
		let url = format!("{}/v1/chat/completions", self.base_url.trim_end_matches('/'));
		let body = build_chat_body(&self.model, messages, tools, false);
		chat_openai_compatible(&client, &url, None, body).await
	}

	async fn stream(
		&self,
		messages: &[AiMessage],
		tools: &[ToolDefinition],
		tx: tokio::sync::mpsc::Sender<StreamEvent>,
	) -> Result<(), ProviderError> {
		let client = reqwest::Client::new();
		let url = format!("{}/v1/chat/completions", self.base_url.trim_end_matches('/'));
		let body = build_chat_body(&self.model, messages, tools, true);
		stream_openai_compatible(&client, &url, None, body, &tx).await
	}
}
