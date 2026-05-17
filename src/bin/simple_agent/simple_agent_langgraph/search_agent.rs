use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TavilyResponse {
    pub answer: Option<String>,
    pub results: Option<Vec<TavilyResult>>,
}

#[derive(Debug, Deserialize)]
pub struct TavilyResult {
    pub title: Option<String>,
    pub content: Option<String>,
    pub url: Option<String>,
}

pub struct TavilyClient {
    client: Client,
    api_key: String,
}

impl TavilyClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    /// 执行 Tavily 搜索
    pub async fn search(&self, query: &str, max_results: usize) -> Result<TavilyResponse> {
        let body = serde_json::json!({
            "api_key": self.api_key,
            "query": query,
            "search_depth": "basic",
            "include_answer": true,
            "include_raw_content": false,
            "max_results": max_results,
        });

        let resp = self.client
            .post("https://api.tavily.com/search")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Tavily 搜索请求失败")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("搜索失败 ({}): {}", status, text));
        }

        let data: TavilyResponse = resp.json().await.context("解析搜索结果失败")?;
        Ok(data)
    }

    /// 格式化搜索结果为文本
    pub fn format_results(response: &TavilyResponse) -> String {
        let mut output = String::new();

        // 优先使用 Tavily 的综合答案
        if let Some(ref answer) = response.answer {
            if !answer.is_empty() {
                output.push_str(&format!("综合答案：\n{}\n\n", answer));
            }
        }

        // 添加具体搜索结果
        if let Some(ref results) = response.results {
            if !results.is_empty() {
                output.push_str("相关信息：\n");
                for (i, result) in results.iter().take(3).enumerate() {
                    let title = result.title.as_deref().unwrap_or("无标题");
                    let content = result.content.as_deref().unwrap_or("无内容");
                    let url = result.url.as_deref().unwrap_or("无链接");
                    output.push_str(&format!("{}. {}\n{}\n来源：{}\n\n", i + 1, title, content, url));
                }
            }
        }

        if output.is_empty() {
            output = "抱歉，没有找到相关信息。".to_string();
        }

        output
    }
}