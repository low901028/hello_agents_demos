use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::future::Future;
use std::pin::Pin;
use anyhow::{Result, Context};
use serde::Deserialize;

// ==================== 百度千帆搜索客户端 ====================
/// 千帆搜索 API 响应
#[derive(Debug, Deserialize)]
struct SearchResponse {
    references: Option<Vec<SearchItem>>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    title: Option<String>,
    url: Option<String>,
    content: Option<String>,
}
pub struct BaiduSearchClient {
    client: Client,
    api_key: String,
    base_url: String,
}

pub async fn search(query: &str) -> Result<String> {
    let bd_api_key = env::var("BAIDU_API_KEY")
        .expect("API key must be provided or set in LLM_API_KEY env var");
    let client = BaiduSearchClient::new(bd_api_key);
    client.search(query).await
}

impl BaiduSearchClient {
    /// 从环境变量 `BAIDU_API_KEY` 创建客户端
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://qianfan.baidubce.com/v2/ai_search/web_search".to_string(),
        }
    }

    /// 执行搜索，返回格式化的结果字符串
    pub async fn search(&self, query: &str) -> Result<String> {
        println!("🔍 正在执行 [百度千帆] 网页搜索: {}", query);

        // 1. 构建请求体，符合千帆平台接口定义[reference:2]
        let request_body = serde_json::json!({
            "messages": [
                {
                    "content": query,
                    "role": "user"
                }
            ],
            "search_source": "baidu_search_v2",
            "resource_type_filter": [{"type": "web","top_k": 10}],
            "search_recency_filter": "year"
        });

        // 2. 发送POST请求
        let response = self
            .client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .header("X-Appbuilder-Authorization", format!("Bearer {}", self.api_key)) // 鉴权头，使用AppBuilder API Key[reference:3]
            .json(&request_body)
            .send()
            .await
            .context("发送搜索请求失败")?;

        // 3. 处理响应
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("搜索请求失败 ({}): {}", status, error_body));
        }

        let search_response: SearchResponse = response.json().await.context("解析搜索结果失败")?;

        // 4. 格式化搜索结果
        match search_response.references {
            Some(items) if !items.is_empty() => {
                let formatted_results: Vec<String> = items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| {
                        format!(
                            "[{}.] {:?}\n    URL: {:?}\n    {:?}\n",
                            i + 1,
                            item.title,
                            item.url,
                            item.content
                        )
                    })
                    .collect();
                Ok(formatted_results.join("\n"))
            }
            _ => Ok(format!("未找到关于 '{}' 的相关信息。", query)),
        }
    }
}


// ----------------- 以下代码保持不变 -----------------
pub type ToolFunc = Box<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync
>;

pub struct ToolExecutor {
    tools: HashMap<String, (String, ToolFunc)>,
}

impl ToolExecutor {
    pub fn new() -> Self {
        ToolExecutor {
            tools: HashMap::new(),
        }
    }

    pub fn register_tool(
        &mut self,
        name: &str,
        description: &str,
        func: ToolFunc,
    ) {
        if self.tools.contains_key(name) {
            println!("警告：工具 '{}' 已存在，将被覆盖。", name);
        }
        self.tools.insert(name.to_string(), (description.to_string(), func));
        println!("工具 '{}' 已注册。", name);
    }

    pub fn get_tool(&self, name: &str) -> Option<&ToolFunc> {
        self.tools.get(name).map(|(_, func)| func)
    }

    pub fn get_available_tools(&self) -> String {
        self.tools
            .iter()
            .map(|(name, (desc, _))| format!("- {}: {}", name, desc))
            .collect::<Vec<_>>()
            .join("\n")
    }
}