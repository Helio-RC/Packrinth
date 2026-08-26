use futures_util::StreamExt;

use crate::ai_workshop::providers::provider_trait::ProviderError;

/// 从 reqwest 响应字节流中解析 SSE 事件，逐行处理 "data: " 前缀的 JSON。
pub async fn parse_sse<R>(
	mut stream: R,
	mut on_data: impl FnMut(serde_json::Value),
) -> Result<(), ProviderError>
where
	R: futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
{
	let mut buffer = String::new();
	while let Some(chunk) = stream.next().await {
		let chunk = chunk.map_err(|e| ProviderError(e.to_string()))?;
		let text = String::from_utf8_lossy(&chunk).replace("\r\n", "\n");
		buffer.push_str(&text);
		while let Some(pos) = buffer.find("\n\n") {
			let event = buffer[..pos].to_string();
			buffer.drain(..pos + 2);
			handle_sse_event(&event, &mut on_data);
		}
	}
	if !buffer.trim().is_empty() {
		handle_sse_event(&buffer, &mut on_data);
	}
	Ok(())
}

fn handle_sse_event(event: &str, on_data: &mut impl FnMut(serde_json::Value)) {
	for line in event.lines() {
		let line = line.trim();
		if let Some(data) = line.strip_prefix("data:") {
			let data = data.trim();
			if data.is_empty() || data == "[DONE]" {
				continue;
			}
			if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
				on_data(value);
			}
		}
	}
}
