// === AI-WORKSHOP START ===
// 技能内容净化（三层，独立以便单测）：
//   第一层  pulldown-cmark 解析为 AST，拒绝任何 HTML 块 / 内联 HTML；
//   第二层  允许的 AST 渲染为 HTML 后经 ammonia 清理 `<script>`、`on*` 事件、`javascript:` 链接；
//   第三层  链接协议限制：拒绝 file:// 与 data:（本地/内联资源访问），其余交由 ammonia 净化。
use pulldown_cmark::{Event, Options, Parser, Tag};

/// 净化 guide.md：成功返回净化后的 HTML；任一校验不通过返回 Err（调用方跳过该技能）。
pub fn sanitize_guide_md(markdown: &str) -> Result<String, String> {
    let options = Options::empty();
    // 第一层 + 第三层：走一遍 AST，拒绝 HTML 块与危险链接协议。
    {
        let parser = Parser::new_ext(markdown, options);
        for event in parser {
            match event {
                Event::Html(_) | Event::InlineHtml(_) => {
                    return Err("不允许嵌入 HTML 块或内联 HTML".to_string());
                }
                Event::Start(Tag::Link { dest_url, .. })
                | Event::Start(Tag::Image { dest_url, .. }) => {
                    if !is_allowed_link(&dest_url) {
                        return Err(format!("不允许的链接协议: {dest_url}"));
                    }
                }
                _ => {}
            }
        }
    }

    // 第二层：允许的 AST 渲染为 HTML，再经 ammonia 清理脚本与危险属性/链接。
    let mut html = String::new();
    let parser = Parser::new_ext(markdown, options);
    pulldown_cmark::html::push_html(&mut html, parser);
    Ok(ammonia::clean(&html))
}

/// 第三层：拒绝允许直接访问本地或内联资源的协议（file/data）。
/// 其余（含 javascript:）交由 ammonia 净化（移除 href），而非拒绝整个技能。
fn is_allowed_link(dest: &str) -> bool {
    let trimmed = dest.trim();
    if trimmed.is_empty() {
        return false;
    }
    let scheme_end = trimmed.find(':').unwrap_or(trimmed.len());
    let maybe_scheme = &trimmed[..scheme_end];
    let is_scheme = !maybe_scheme.is_empty()
        && maybe_scheme
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        && maybe_scheme.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.'
        });
    if !is_scheme {
        // 相对链接（无协议），允许。
        return true;
    }
    !matches!(maybe_scheme.to_ascii_lowercase().as_str(), "file" | "data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_raw_script_block() {
        let md = "hello\n\n<script>alert(1)</script>\n";
        assert!(sanitize_guide_md(md).is_err());
    }

    #[test]
    fn cleans_javascript_link() {
        let md = "[click](javascript:alert(1))";
        let result = sanitize_guide_md(md).unwrap();
        assert!(!result.to_lowercase().contains("javascript:"));
    }

    #[test]
    fn rejects_img_onerror() {
        let md = "<img src=x onerror=alert(1)>";
        assert!(sanitize_guide_md(md).is_err());
    }

    #[test]
    fn rejects_file_and_data_links() {
        assert!(sanitize_guide_md("[x](file:///etc/passwd)").is_err());
        assert!(sanitize_guide_md("[x](data:text/html,<script>)").is_err());
    }

    #[test]
    fn accepts_normal_markdown() {
        let md = "# Hello\n\nThis is a **guide** with a [link](https://example.com).";
        let result = sanitize_guide_md(md).unwrap();
        assert!(result.contains("Hello"));
        assert!(result.contains("https://example.com"));
    }
}
// === AI-WORKSHOP END ===
