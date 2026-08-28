// === AI-WORKSHOP START ===
// 知识原子工具：crawl_document 抓取网页并分块，供 AI 引擎检索外部文档。
// 域名白名单在构造时固定传入（crawl_document 无 config 访问，白名单用默认值常量）。
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use super::context::ExecutionContext;
use super::registry::{Tool, ToolDomain, ToolInfo};
use crate::ai_workshop::knowledge::crawler::{
    DEFAULT_ALLOWED_DOMAINS, UrlFilter,
};

/// 从 arguments 中读取字符串参数；缺失或类型不符返回错误。
fn string_arg(arguments: &Value, key: &str) -> Result<String, String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("缺少参数: {key}"))
}

/// 抓取网页并分块（readonly）。参数：url 必填，max_chars 默认 2000。
/// 执行流程：UrlFilter 白名单校验 → fetch_and_extract → chunk_content → 返回 { title, url, chunks }。
pub struct CrawlDocumentTool {
    allowed_domains: Vec<String>,
}

impl CrawlDocumentTool {
    pub fn new(allowed_domains: Vec<String>) -> Self {
        Self { allowed_domains }
    }
}

impl Default for CrawlDocumentTool {
    fn default() -> Self {
        Self::new(
            DEFAULT_ALLOWED_DOMAINS
                .iter()
                .map(|d| d.to_string())
                .collect(),
        )
    }
}

#[async_trait]
impl Tool for CrawlDocumentTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "crawl_document".to_string(),
            description:
                "抓取网页正文并分块，供检索外部文档（仅限白名单域名）。"
                    .to_string(),
            domain: ToolDomain::Knowledge,
            requires_confirmation: false,
            is_readonly: true,
            params_schema: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "要抓取的页面 URL" },
                    "max_chars": { "type": "integer", "default": 2000, "description": "每块最大字符数，默认 2000" }
                },
                "required": ["url"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        _ctx: &ExecutionContext,
    ) -> Result<Value, String> {
        let url = string_arg(&arguments, "url")?;
        let max_chars = arguments
            .get("max_chars")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(2000);

        let filter = UrlFilter::new(self.allowed_domains.clone());
        let docs = crate::ai_workshop::knowledge::crawler::crawl(
            &url, &filter, max_chars,
        )
        .await
        .map_err(|e| e.to_string())?;

        let title = docs.first().map(|d| d.title.clone()).unwrap_or_default();
        let chunks: Vec<String> = docs.into_iter().map(|d| d.content).collect();
        Ok(json!({ "title": title, "url": url, "chunks": chunks }))
    }
}

/// 构造并注册全部知识工具。
pub fn register_knowledge_tools(registry: &Arc<super::registry::ToolRegistry>) {
    registry.register(Arc::new(CrawlDocumentTool::default()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn crawl_document_rejects_non_whitelisted_domain_without_network() {
        let tool = CrawlDocumentTool::new(vec!["modrinth.com".to_string()]);
        // 域名不在白名单：在发起任何网络请求前即返回 Err。
        let result = tool
            .execute(
                json!({ "url": "https://example.com/docs" }),
                &ExecutionContext::default(),
            )
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("白名单"));
    }

    #[tokio::test]
    async fn crawl_document_requires_url() {
        let tool = CrawlDocumentTool::default();
        let result =
            tool.execute(json!({}), &ExecutionContext::default()).await;
        assert_eq!(result.unwrap_err(), "缺少参数: url");
    }

    #[tokio::test]
    async fn crawl_document_rejects_invalid_url_without_network() {
        let tool = CrawlDocumentTool::new(vec!["modrinth.com".to_string()]);
        // 非法 URL：UrlFilter 解析失败 → 视为不允许 → Err。
        let result = tool
            .execute(
                json!({ "url": "not-a-url" }),
                &ExecutionContext::default(),
            )
            .await;
        assert!(result.is_err());
    }

    fn spawn_test_server(body: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut buf = [0u8; 4096];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                break;
            }
        });
        format!("http://{addr}/")
    }

    #[tokio::test]
    async fn crawl_document_fetches_and_chunks() {
        // 本地服务器成功路径：白名单含 127.0.0.1，抓取 → 分块 → { title, url, chunks }。
        let url = spawn_test_server(
            "<html><head><title>Doc</title></head><body><article><p>alpha beta gamma.</p></article></body></html>",
        );
        let tool = CrawlDocumentTool::new(vec!["127.0.0.1".to_string()]);
        let value = tool
            .execute(json!({ "url": url }), &ExecutionContext::default())
            .await
            .expect("crawl should succeed on local server");
        assert_eq!(value["title"], json!("Doc"));
        assert_eq!(value["url"], json!(url));
        let chunks = value["chunks"].as_array().unwrap();
        assert!(!chunks.is_empty());
        assert!(chunks[0].as_str().unwrap().contains("alpha"));
    }
}
// === AI-WORKSHOP END ===
