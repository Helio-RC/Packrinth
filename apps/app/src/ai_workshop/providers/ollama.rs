// === AI-WORKSHOP START ===
// Ollama 本地模型提供商：内部复用 OpenAIProvider（兼容 /v1/chat/completions，无鉴权）。
use crate::ai_workshop::providers::openai::OpenAIProvider;
use crate::ai_workshop::providers::provider_trait::{
    AiMessage, AiProvider, AiResponse, ProviderError, StreamEvent,
    ToolDefinition,
};

/// Ollama 本地模型提供商（兼容 OpenAI 端点）。
pub struct OllamaProvider {
    inner: OpenAIProvider,
}

impl OllamaProvider {
    pub fn new(model: String, base_url: Option<String>) -> Self {
        let mut base =
            base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
        if !base.ends_with("/v1") {
            base = format!("{}/v1", base.trim_end_matches('/'));
        }
        Self {
            inner: OpenAIProvider::new(String::new(), model, Some(base)),
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
        self.inner.chat(messages, tools).await
    }

    async fn stream(
        &self,
        messages: &[AiMessage],
        tools: &[ToolDefinition],
        tx: tokio::sync::mpsc::Sender<StreamEvent>,
    ) -> Result<(), ProviderError> {
        self.inner.stream(messages, tools, tx).await
    }
}
// === AI-WORKSHOP END ===
