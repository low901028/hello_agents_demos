// examples/weather_tools_demo.rs

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;

use hello_agents::core::traits::tool::Tool;
use hello_agents::core::traits::agent::Agent;
use hello_agents::core::agent_runtime::AgentRuntime;
use hello_agents::infra::openai_adapter::OpenAIAdapter;
use hello_agents::infra::tool_registry_impl::ToolRegistryImpl;
use hello_agents::context::history_manager_impl::HistoryManagerImpl;
use hello_agents::agents::simple::SimpleAgent;
use hello_agents::core::traits::history::HistoryManager;
use hello_agents::core::traits::tool_registry::ToolRegistry;
use hello_agents::core::types::config::Config;
use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::core::types::response::ToolResponse;

// ==================== 搜索工具 (异步百度千帆) ====================
struct SearchTool {
    client: Client,
    api_key: String,
}

impl SearchTool {
    fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    async fn search_online(&self, query: &str) -> Result<String, HelloAgentError> {
        let resp = self.client
            .post("https://qianfan.baidubce.com/v2/ai_search/web_search")
            .header("X-Appbuilder-Authorization", format!("Bearer {}", self.api_key))
            .json(&json!({
                "messages": [{"content": query, "role": "user"}],
                "search_source": "baidu_search_v2",
                "resource_type_filter": [{"type": "web", "top_k": 10}],
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(HelloAgentError::General("搜索请求失败".into()));
        }

        let json: Value = resp.json().await?;
        let references = json["references"].as_array().cloned().unwrap_or_default();
        let results: Vec<String> = references.iter().filter_map(|r| {
            let title = r["title"].as_str().unwrap_or("");
            let url = r["url"].as_str().unwrap_or("");
            let content = r["content"].as_str().unwrap_or("");
            Some(format!("[{}]({}) - {}", title, url, content))
        }).collect();

        if results.is_empty() {
            Ok(format!("未找到关于 '{}' 的相关信息。", query))
        } else {
            Ok(results.join("\n"))
        }
    }
}

#[async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &str { "Search" }
    fn description(&self) -> &str { "搜索互联网信息" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索关键词"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let result = self.search_online(query).await?;
        Ok(ToolResponse::success(result))
    }
}

// ==================== 计算器工具 ====================
struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str { "Calculator" }
    fn description(&self) -> &str { "执行数学计算" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "数学表达式"
                }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let expr = args.get("expression").and_then(|v| v.as_str()).unwrap_or("");
        match meval::eval_str(expr) {
            Ok(result) => Ok(ToolResponse::success(format!("计算结果：{} = {}", expr, result))),
            Err(e) => Ok(ToolResponse::error("CALC_ERROR", &format!("计算失败: {}", e))),
        }
    }
}

// ==================== 天气工具 ====================
struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str { "Weather" }
    fn description(&self) -> &str { "查询指定城市的实时天气信息" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "城市名称"
                },
                "unit": {
                    "type": "string",
                    "description": "温度单位 (celsius/fahrenheit)"
                }
            },
            "required": ["city"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let city = args.get("city").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let unit = args.get("unit").and_then(|v| v.as_str()).unwrap_or("celsius");
        // 模拟天气数据
        let mock_data = json!({
            "北京": {"temp": 15, "description": "晴天"},
            "上海": {"temp": 20, "description": "多云"},
        });
        let weather = mock_data.get(&city).ok_or_else(|| HelloAgentError::Tool("城市不存在".into()))?;
        let temp = weather["temp"].as_f64().unwrap_or(0.0);
        let desc = weather["description"].as_str().unwrap_or("");
        let display_temp = if unit == "fahrenheit" { temp * 9.0 / 5.0 + 32.0 } else { temp };
        Ok(ToolResponse::success(format!(
            "{} 天气：温度 {:.1}{}, {}",
            city, display_temp, if unit == "celsius" { "°C" } else { "°F" }, desc
        )))
    }
}

// ==================== 主函数 ====================
#[tokio::main]
async fn main() -> Result<(), HelloAgentError> {
    dotenvy::dotenv().ok();

    // 1. 创建 LLM 提供商
    let llm = Arc::new(OpenAIAdapter::new(
        &std::env::var("LLM_API_KEY").unwrap(),
        &std::env::var("LLM_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".into()),
        &std::env::var("LLM_MODEL_ID").unwrap_or_else(|_| "deepseek-chat".into()),
    ));

    // 2. 注册工具
    let mut tools = ToolRegistryImpl::new();
    let baidu_key = std::env::var("BAIDU_API_KEY").expect("BAIDU_API_KEY not set");
    tools.register(Box::new(SearchTool::new(baidu_key)));
    tools.register(Box::new(CalculatorTool));
    tools.register(Box::new(WeatherTool));
    let tools = Arc::new(tools);

    // 3. 历史管理器
    let history = Arc::new(std::sync::Mutex::new(
        Box::new(HistoryManagerImpl::new(10, 0.8)) as Box<dyn HistoryManager>,
    ));
    // 4. 配置
    let config = Config::default();

    // 5. 组装运行时
    let runtime = AgentRuntime::new(llm, tools, history, config);

    // 6. 创建 Agent
    let mut agent = SimpleAgent::new(
        "AsyncAgent",
        Some("你是一个有用的AI助手".into()),
        5,
    );

    // 7. 执行
    let result = agent.run(
        "搜索 Python 异步编程的资料，并计算 123 + 456，同时查询北京的天气",
        &runtime,
    ).await?;

    println!("最终结果: {}", result);
    Ok(())
}