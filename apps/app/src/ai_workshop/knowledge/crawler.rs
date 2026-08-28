// === AI-WORKSHOP START ===
// 内容爬取：域名白名单过滤 + scraper HTML 提取（main/article，fallback body）+ 智能分块。
use std::time::SystemTime;

use scraper::{Html, Selector};
use url::Url;

use super::chunker::chunk_content;
use super::source::SourceDocument;
use crate::ai_workshop::other_err;
use crate::api::Result;

/// 响应体大小上限：2MB。
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
/// 默认白名单域名，与 config.rs 中 AiWorkshopConfig::default().knowledge.allowed_domains 保持一致。
/// crawl_document 工具在构造时固定使用此默认值（工具无 config 访问）。
pub const DEFAULT_ALLOWED_DOMAINS: [&str; 4] = [
    "modrinth.com",
    "mcmod.cn",
    "minecraft.fandom.com",
    "ftbwiki.org",
];
/// 默认 User-Agent，避免被站点反爬拦截。
const USER_AGENT: &str = "ModrinthAppBot/1.0 (+https://modrinth.com)";

/// 提取到的页面内容。
pub struct FetchedPage {
    pub url: String,
    pub title: String,
    pub content: String,
}

/// 域名白名单过滤：精确匹配 host 或 host 以 `.白名单域名` 结尾（允许子域）。
pub struct UrlFilter {
    allowed_domains: Vec<String>,
}

impl UrlFilter {
    pub fn new(allowed_domains: Vec<String>) -> Self {
        let normalized = allowed_domains
            .into_iter()
            .map(|d| {
                d.trim()
                    .trim_start_matches("https://")
                    .trim_start_matches("http://")
                    .to_lowercase()
            })
            .filter(|d| !d.is_empty())
            .collect();
        Self {
            allowed_domains: normalized,
        }
    }

    pub fn is_allowed(&self, url: &str) -> bool {
        let Some(host) = Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
        else {
            return false;
        };
        self.allowed_domains.iter().any(|domain| {
            host == *domain || host.ends_with(&format!(".{domain}"))
        })
    }
}

/// 获取 HTML 并提取正文：优先 main/article 节点文本，fallback body 全部文本。
pub async fn fetch_and_extract(url: &str) -> Result<FetchedPage> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(err)?;

    let response = client.get(url).send().await.map_err(err)?;
    if !response.status().is_success() {
        return Err(err(format!("HTTP {}: {url}", response.status())));
    }
    if let Some(ct) = response.headers().get(reqwest::header::CONTENT_TYPE) {
        let ct = ct.to_str().unwrap_or("").to_lowercase();
        let is_html = ct.contains("text/html")
            || ct.contains("+html")
            || ct.contains("application/xhtml");
        if !is_html {
            return Err(err(format!("非 HTML 内容类型: {ct}")));
        }
    }
    if let Some(len) = response.content_length()
        && len > MAX_BODY_BYTES as u64
    {
        return Err(err(format!("响应体超过 {MAX_BODY_BYTES} 字节限制")));
    }
    let body = response.bytes().await.map_err(err)?;
    if body.len() > MAX_BODY_BYTES {
        return Err(err(format!("响应体超过 {MAX_BODY_BYTES} 字节限制")));
    }

    let html = String::from_utf8_lossy(&body);
    let document = Html::parse_document(&html);
    let title = extract_title(&document);
    // html2md 整体转 Markdown（保留标题层级/表格）；空结果回退到 scraper 文本提取。
    let content = {
        let markdown = html2md::parse_html(&html);
        if markdown.trim().is_empty() {
            extract_content(&document)
        } else {
            normalize_text(&markdown)
        }
    };
    Ok(FetchedPage {
        url: url.to_string(),
        title,
        content,
    })
}

/// 校验域名 → 抓取 → 分块 → SourceDocument 列表（path=url，mtime=now）。
pub async fn crawl(
    url: &str,
    filter: &UrlFilter,
    max_chars: usize,
) -> Result<Vec<SourceDocument>> {
    if !filter.is_allowed(url) {
        return Err(other_err(format!("域名不在白名单内: {url}")));
    }
    let page = fetch_and_extract(url).await?;
    let chunks = chunk_content(&page.title, &page.content, max_chars);
    let now = SystemTime::now();
    Ok(chunks
        .into_iter()
        .map(|c| SourceDocument {
            title: page.title.clone(),
            content: c,
            path: page.url.clone(),
            mtime: Some(now),
        })
        .collect())
}

/// 提取标题：title 标签优先，fallback h1。
fn extract_title(document: &Html) -> String {
    for sel in ["title", "h1"] {
        if let Ok(selector) = Selector::parse(sel)
            && let Some(el) = document.select(&selector).next()
        {
            let text = el.text().collect::<Vec<_>>().join(" ");
            let collapsed = collapse_whitespace(&text);
            if !collapsed.is_empty() {
                return collapsed;
            }
        }
    }
    String::new()
}

/// 提取正文：main/article 节点文本，fallback body。
fn extract_content(document: &Html) -> String {
    for sel in ["main", "article"] {
        if let Ok(selector) = Selector::parse(sel) {
            let parts: Vec<String> = document
                .select(&selector)
                .map(|el| el.text().collect::<Vec<_>>().join(" "))
                .collect();
            if !parts.is_empty() {
                return normalize_text(&parts.join("\n"));
            }
        }
    }
    if let Ok(selector) = Selector::parse("body")
        && let Some(el) = document.select(&selector).next()
    {
        return normalize_text(&el.text().collect::<Vec<_>>().join(" "));
    }
    normalize_text(
        &document.root_element().text().collect::<Vec<_>>().join(" "),
    )
}

/// 折叠行内空白为单个空格，并以换行保留段间分隔（空行段落折叠）。
fn normalize_text(text: &str) -> String {
    let paragraphs: Vec<String> = text
        .lines()
        .map(collapse_whitespace)
        .filter(|l| !l.is_empty())
        .collect();
    paragraphs.join("\n")
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn err(e: impl std::fmt::Display) -> crate::api::TheseusSerializableError {
    other_err(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn filter(domains: &[&str]) -> UrlFilter {
        UrlFilter::new(domains.iter().map(|d| d.to_string()).collect())
    }

    #[test]
    fn whitelist_exact_and_subdomain() {
        let f = filter(&["modrinth.com"]);
        assert!(f.is_allowed("https://modrinth.com/"));
        assert!(f.is_allowed("https://docs.modrinth.com/guides"));
        assert!(f.is_allowed("https://a.b.modrinth.com/x"));
        assert!(!f.is_allowed("https://evilmodrinth.com/"));
        assert!(!f.is_allowed("https://example.com/"));
        assert!(!f.is_allowed("not-a-url"));
    }

    #[test]
    fn port_is_ignored_for_host_match() {
        let f = filter(&["localhost"]);
        assert!(f.is_allowed("http://localhost:8080/page"));
        assert!(!f.is_allowed("http://evilhost:8080/page"));
    }

    #[test]
    fn scheme_and_whitespace_in_whitelist_normalized() {
        let f = filter(&[" https://Modrinth.com "]);
        assert!(f.is_allowed("https://modrinth.com/"));
    }

    fn spawn_test_server(body: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Some(stream) = listener.incoming().flatten().next() {
                let mut stream = stream;
                // 先消费请求，hyper 才会接受随后写出的响应（否则报 UnexpectedMessage）。
                let mut buf = [0u8; 4096];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}/")
    }

    fn spawn_oversized_server() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Some(stream) = listener.incoming().flatten().next() {
                let mut stream = stream;
                let mut buf = [0u8; 4096];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                // 声明超限的 Content-Length，但无需真正发送 body。
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    MAX_BODY_BYTES as u64 + 1
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}/")
    }

    #[tokio::test]
    async fn fetch_and_extract_extracts_main_content() {
        let url = spawn_test_server(
            "<html><head><title>Test Page</title></head><body><article><p>Hello world.</p><p>Second para.</p></article></body></html>",
        );
        let page = fetch_and_extract(&url).await.unwrap();
        assert_eq!(page.title, "Test Page");
        assert!(page.content.contains("Hello world."));
        assert!(page.content.contains("Second para."));
    }

    #[tokio::test]
    async fn fetch_and_extract_falls_back_to_body() {
        let url = spawn_test_server(
            "<html><head><title>No Main</title></head><body><p>Only body text here.</p></body></html>",
        );
        let page = fetch_and_extract(&url).await.unwrap();
        assert_eq!(page.title, "No Main");
        assert!(page.content.contains("Only body text here."));
    }

    #[tokio::test]
    async fn oversized_body_is_rejected() {
        let url = spawn_oversized_server();
        let res = fetch_and_extract(&url).await;
        assert!(res.is_err(), "oversized response must be rejected");
    }

    fn spawn_custom_server(
        status: &'static str,
        content_type: &'static str,
        body: &'static str,
    ) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Some(stream) = listener.incoming().flatten().next() {
                let mut stream = stream;
                let mut buf = [0u8; 4096];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}/")
    }

    #[tokio::test]
    async fn non_success_status_is_rejected() {
        let url = spawn_custom_server(
            "404 Not Found",
            "text/html",
            "<html><body><p>Not found</p></body></html>",
        );
        let res = fetch_and_extract(&url).await;
        assert!(res.is_err(), "4xx/5xx must be rejected");
    }

    #[tokio::test]
    async fn non_html_content_type_is_rejected() {
        let url = spawn_custom_server(
            "200 OK",
            "application/json",
            r#"{"key":"value"}"#,
        );
        let res = fetch_and_extract(&url).await;
        assert!(res.is_err(), "non-HTML content type must be rejected");
    }

    #[tokio::test]
    async fn xhtml_content_type_is_accepted() {
        let url = spawn_custom_server(
            "200 OK",
            "application/xhtml+xml",
            "<html><head><title>Xhtml</title></head><body><p>ok</p></body></html>",
        );
        let page = fetch_and_extract(&url).await.unwrap();
        assert!(page.content.contains("ok"));
    }

    #[tokio::test]
    async fn crawl_checks_domain_then_chunks() {
        let url = spawn_test_server(
            "<html><head><title>Crawl Me</title></head><body><article><p>alpha beta gamma delta epsilon.</p><p>zeta eta theta.</p></article></body></html>",
        );
        let f = filter(&["127.0.0.1"]);
        let docs = crawl(&url, &f, 2000).await.unwrap();
        assert!(!docs.is_empty());
        assert!(docs.iter().all(|d| d.title == "Crawl Me"));
        assert!(docs.iter().all(|d| d.mtime.is_some()));
        assert_eq!(docs[0].path, url);

        // 不在白名单 → 拒绝。
        let denied = filter(&["example.com"]);
        assert!(crawl(&url, &denied, 2000).await.is_err());
    }
}
// === AI-WORKSHOP END ===
