// === AI-WORKSHOP START ===
// 智能分块：按段落/句子边界切分文档，每块不超过 max_chars（≈512 tokens 的保守字符估算）。
// 不做真实 token 化；title 由调用方附加到 SourceDocument，本模块只返回纯文本片段。
/// 将 content 按空行分段、超大段再按句子/单词切分，返回每块 ≤ max_chars 的文本块列表。
pub fn chunk_content(_title: &str, content: &str, max_chars: usize) -> Vec<String> {
	if content.trim().is_empty() || max_chars == 0 {
		return Vec::new();
	}
	let paragraphs = split_paragraphs(content);
	let mut chunks: Vec<String> = Vec::new();
	let mut current = String::new();

	for para in paragraphs {
		// 能塞进当前块则追加。
		if !current.is_empty() && current.len() + 1 + para.len() <= max_chars {
			current.push('\n');
			current.push_str(para);
			continue;
		}
		// 当前块已满，段落本身不超限 → 作为新块起点。
		if para.len() <= max_chars {
			if !current.is_empty() {
				chunks.push(std::mem::take(&mut current));
			}
			current = para.to_string();
			continue;
		}
		// 段落自身超限：先落当前块，再对段落做句/词级切分。
		if !current.is_empty() {
			chunks.push(std::mem::take(&mut current));
		}
		chunks.extend(split_oversized(para, max_chars));
	}

	if !current.is_empty() {
		chunks.push(current);
	}
	chunks
}

/// 按空行分段；段落内部空白折叠为单个空格。
fn split_paragraphs(text: &str) -> Vec<&str> {
	text.split('\n')
		.map(|line| line.trim())
		.filter(|line| !line.is_empty())
		.collect()
}

/// 对超长段落：优先按句号等句子边界切分，句内再按词边界硬切（保持语义完整优先）。
fn split_oversized(para: &str, max_chars: usize) -> Vec<String> {
	let sentences = split_sentences(para);
	let mut chunks: Vec<String> = Vec::new();
	let mut buf = String::new();

	for sent in sentences {
		let sent = sent.trim();
		if sent.is_empty() {
			continue;
		}
		if buf.len() + 1 + sent.len() <= max_chars {
			if !buf.is_empty() {
				buf.push(' ');
			}
			buf.push_str(sent);
		} else {
			if !buf.is_empty() {
				chunks.push(std::mem::take(&mut buf));
			}
			if sent.len() <= max_chars {
				buf = sent.to_string();
			} else {
				chunks.extend(split_words(sent, max_chars));
			}
		}
	}
	if !buf.is_empty() {
		chunks.push(buf);
	}
	chunks
}

/// 按句子边界切分：`.` `!` `?` 与中文句末标点后跟空白/结尾处断开。
fn split_sentences(text: &str) -> Vec<&str> {
	let mut result: Vec<&str> = Vec::new();
	let mut start = 0usize;
	for (i, c) in text.char_indices() {
		let is_term = matches!(c, '.' | '!' | '?' | '。' | '！' | '？');
		if !is_term {
			continue;
		}
		let after = i + c.len_utf8();
		// 中文句末标点无需空白即断句；ASCII 句末标点须后跟空白或结尾，避免小数/缩写被误切。
		let is_cjk = matches!(c, '。' | '！' | '？');
		let boundary = after >= text.len() || is_cjk || text[after..].starts_with(char::is_whitespace);
		if boundary {
			result.push(&text[start..after]);
			start = after;
		}
	}
	if start < text.len() {
		result.push(&text[start..]);
	}
	result
}

/// 单句仍超限时的兜底：按空白词边界切分；单个词（如无空格的中文长串）再按字符硬切。
fn split_words(text: &str, max_chars: usize) -> Vec<String> {
	let mut chunks: Vec<String> = Vec::new();
	let mut buf = String::new();
	for w in text.split_whitespace() {
		if buf.is_empty() {
			if w.len() <= max_chars {
				buf.push_str(w);
			} else {
				chunks.extend(split_chars(w, max_chars));
			}
		} else if buf.len() + 1 + w.len() <= max_chars {
			buf.push(' ');
			buf.push_str(w);
		} else {
			chunks.push(std::mem::take(&mut buf));
			if w.len() <= max_chars {
				buf.push_str(w);
			} else {
				chunks.extend(split_chars(w, max_chars));
			}
		}
	}
	if !buf.is_empty() {
		chunks.push(buf);
	}
	chunks
}

/// 单个超长 token 按字符边界切分（UTF-8 安全）。
fn split_chars(text: &str, max_chars: usize) -> Vec<String> {
	let mut chunks: Vec<String> = Vec::new();
	let mut buf = String::new();
	for c in text.chars() {
		if buf.len() + c.len_utf8() <= max_chars {
			buf.push(c);
		} else {
			chunks.push(std::mem::take(&mut buf));
			buf.push(c);
		}
	}
	if !buf.is_empty() {
		chunks.push(buf);
	}
	chunks
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn empty_input_returns_empty() {
		assert!(chunk_content("t", "", 2000).is_empty());
		assert!(chunk_content("t", "   \n  ", 2000).is_empty());
		assert!(chunk_content("t", "hello", 0).is_empty());
	}

	#[test]
	fn short_content_is_single_chunk() {
		let chunks = chunk_content("t", "Hello world. This is short.", 2000);
		assert_eq!(chunks.len(), 1);
		assert_eq!(chunks[0], "Hello world. This is short.");
	}

	#[test]
	fn long_content_split_into_multiple_chunks() {
		let para = "这是一个很长的段落，包含许多句子。每句都以句号结尾。" .repeat(200);
		let content = format!("{para}\n\n{para}");
		let chunks = chunk_content("t", &content, 2000);
		assert!(chunks.len() > 1, "long content must yield multiple chunks");
		for c in &chunks {
			assert!(c.len() <= 2000, "chunk exceeds max_chars: {}", c.len());
		}
	}

	#[test]
	fn chunks_respect_max_chars_boundary() {
		let content = "word ".repeat(500);
		let chunks = chunk_content("t", &content, 100);
		assert!(chunks.len() >= 2);
		for c in &chunks {
			assert!(c.len() <= 100);
		}
	}

	#[test]
	fn spaceless_long_token_split_within_max_chars() {
		let token = "漢".repeat(3000);
		let chunks = chunk_content("t", &token, 2000);
		assert!(chunks.len() > 1, "oversized token must yield multiple chunks");
		for c in &chunks {
			assert!(c.len() <= 2000, "chunk exceeds max_chars: {}", c.len());
		}
	}

	#[test]
	fn paragraphs_preserved_in_chunks() {
		let chunks = chunk_content("t", "paragraph one\n\nparagraph two", 2000);
		assert_eq!(chunks.len(), 1);
		assert!(chunks[0].contains("paragraph one"));
		assert!(chunks[0].contains("paragraph two"));
	}
}
// === AI-WORKSHOP END ===