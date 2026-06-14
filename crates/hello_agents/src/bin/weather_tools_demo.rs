// examples/async_agent_demo.rs 或 src/main.rs

use std::collections::HashMap;
use std::env;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures::Future;
use serde_json::{json, Value};
use anyhow::{Result, Context};
use reqwest::Client;
use serde::Deserialize;
use hello_agents::agents::react::ReActAgent;
use hello_agents::core::traits::agent::Agent;
use hello_agents::core::traits::lifecycle::LifecycleHook;
use hello_agents::core::traits::tool::{Tool, ToolParameter};
use hello_agents::core::types::config::Config;
use hello_agents::core::types::event::{AgentEvent, EventType};
use hello_agents::core::types::exceptions::HelloAgentException;
use hello_agents::infra::llm::hello_agents_llm::HelloAgentsLLM;
use hello_agents::tools::registry::ToolRegistry;
use hello_agents::tools::response::{ToolResponse, ToolStatus};
use hello_agents::tools::tool_base::ToolBase;

// ------------------------------------------------------------
// 示例工具：搜索工具
// ------------------------------------------------------------
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
pub type ToolFunc = Box<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync
>;
fn create_search_tool() -> ToolFunc {
    Box::new(move |input: String| -> Pin<Box<dyn Future<Output = String> + Send>> {
        Box::pin(async move {
            search(&input).await.unwrap()
        })
    })
}
struct SearchTool {
    base: ToolBase,
}

impl SearchTool {
    fn new() -> Self {
        Self {
            base: ToolBase::new("Search", "搜索互联网信息", false),
        }
    }
}

impl Tool for SearchTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let query = parameters.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let _= create_search_tool();
        Ok(ToolResponse::success(
            format!("搜索结果：关于 '{}' 的信息...", query),
            Some(json!({"query": query, "results": 10})),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new("query", "string", "搜索关键词", true, None)]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone() })
    }
}

// ------------------------------------------------------------
// 示例工具：计算器工具（使用 meval）
// ------------------------------------------------------------
struct CalculatorTool {
    base: ToolBase,
}

impl CalculatorTool {
    fn new() -> Self {
        Self {
            base: ToolBase::new("Calculator", "执行数学计算", false),
        }
    }
}

impl Tool for CalculatorTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let expr = parameters.get("expression").and_then(|v| v.as_str()).unwrap_or("");
        match meval::eval_str(expr) {
            Ok(result) => Ok(ToolResponse::success(
                format!("计算结果：{} = {}", expr, result),
                Some(json!({"expression": expr, "result": result})),
                None,
                None,
            )),
            Err(e) => Ok(ToolResponse::error(
                "CALC_ERROR",
                &format!("计算失败: {}", e),
                None,
                None,
            )),
        }
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new("expression", "string", "数学表达式", true, None)]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone() })
    }
}

// ------------------------------------------------------------
// 示例工具：天气工具（模拟）
// ------------------------------------------------------------
struct WeatherTool {
    base: ToolBase,
    cache: Mutex<HashMap<String, (std::time::Instant, Value)>>,
}

impl WeatherTool {
    fn new() -> Self {
        Self {
            base: ToolBase::new("Weather", "查询指定城市的实时天气信息", false),
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn fetch_weather(&self, city: &str, unit: &str) -> Option<Value> {
        let mut mock: HashMap<&str, Value> = HashMap::new();
        mock.insert("北京", json!({"temp": 15.0, "description": "晴天", "humidity": 45, "wind_speed": 12.0}));
        mock.insert("上海", json!({"temp": 20.0, "description": "多云", "humidity": 60, "wind_speed": 8.0}));
        let data = mock.get(city)?;
        let mut data = data.clone();
        if unit == "fahrenheit" {
            if let Some(temp) = data["temp"].as_f64() {
                let f = temp * 9.0 / 5.0 + 32.0;
                data["temp"] = json!(f);
            }
        }
        Some(json!({
            "city": city,
            "temperature": data["temp"],
            "unit": unit,
            "description": data["description"],
            "humidity": data["humidity"],
            "wind_speed": data["wind_speed"]
        }))
    }
}

impl Tool for WeatherTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let city = parameters.get("city").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if city.is_empty() {
            return Ok(ToolResponse::error("INVALID_PARAM", "城市不能为空", None, None));
        }
        let unit = parameters.get("unit").and_then(|v| v.as_str()).unwrap_or("celsius");
        let weather = self.fetch_weather(&city, unit).ok_or_else(|| HelloAgentException::tool("城市不存在"))?;
        Ok(ToolResponse::success(
            format!("{} 天气：温度 {:.1}{}, {}", city, weather["temperature"].as_f64().unwrap(), if unit == "celsius" { "°C" } else { "°F" }, weather["description"].as_str().unwrap()),
            Some(weather),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("city", "string", "城市名称", true, None),
            ToolParameter::new("unit", "string", "celsius 或 fahrenheit", false, Some(Value::String("celsius".into()))),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone(), cache: Mutex::new(HashMap::new()) })
    }
}

/// ------------------------------------------------------------
/// 生命周期钩子
/// ------------------------------------------------------------
fn make_hook<F>(f: F) -> LifecycleHook
where
    F: Fn(AgentEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync + 'static,
{
    Arc::new(f)
}

// -------- 生命周期钩子直接定义为闭包 ----------
fn on_agent_start(event: AgentEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        println!("\n🚀 [{}] 开始执行", event.agent_name);
        if let Some(input) = event.data.get("input_text") {
            println!("   输入: {}", input.as_str().unwrap_or(""));
        }
    })
}

fn on_agent_finish(event: AgentEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        let result = event.data.get("result").map(|v| v.as_str().unwrap_or("")).unwrap_or("");
        println!("\n✅ [{}] 执行完成", event.agent_name);
        println!("   结果: {}", result);
    })
}

fn on_error(event: AgentEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        let error = event.data.get("error").map(|v| v.as_str().unwrap_or("")).unwrap_or("");
        println!("\n❌ 错误: {}", error);
    })
}

// -------- 异步执行 Agent 的通用函数 ----------
async fn run_agent_async<A: Agent + 'static + std::marker::Send>(
    agent: Arc<Mutex<A>>,
    input: &str,
    on_start: Option<LifecycleHook>,
    on_finish: Option<LifecycleHook>,
    on_error: Option<LifecycleHook>,
    kwargs: HashMap<String, String>,
) -> Result<String, HelloAgentException> {
    let input = input.to_owned();

    if let Some(ref hook) = on_start {
        let event = AgentEvent::new(
            EventType::AgentStart,
            "AsyncAgent".into(),
            { let mut d = HashMap::new(); d.insert("input_text".into(), Value::String(input.clone())); d },
        );
        (hook)(event).await;
    }

    let agent_clone = agent.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut guard = agent_clone.lock().unwrap();
        guard.run(&input, kwargs)
    })
        .await
        .map_err(|e| HelloAgentException::llm(format!("异步任务失败: {}", e)))?;

    match &result {
        Ok(res) => {
            if let Some(ref hook) = on_finish {
                let event = AgentEvent::new(
                    EventType::AgentFinish,
                    "AsyncAgent".into(),
                    { let mut d = HashMap::new(); d.insert("result".into(), Value::String(res.clone())); d },
                );
                (hook)(event).await;
            }
        }
        Err(e) => {
            if let Some(ref hook) = on_error {
                let event = AgentEvent::new(
                    EventType::AgentError,
                    "AsyncAgent".into(),
                    { let mut d = HashMap::new(); d.insert("error".into(), Value::String(e.to_string())); d },
                );
                (hook)(event).await;
            }
        }
    }

    result
}

// ------------------------------------------------------------
// 主函数
// ------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    dotenvy::dotenv().ok();
    let llm = HelloAgentsLLM::new(None, None, None, None, None, None, None)?;
    let adapter = llm.adapter();

    let mut registry = ToolRegistry::new(None);
    registry.register_tool(Box::new(SearchTool::new()), false);
    // registry.register_tool(Box::new(CalculatorTool::new()), false);
    // registry.register_tool(Box::new(WeatherTool::new()), false);
    let registry = Arc::new(Mutex::new(registry));

    let config = Config {
        max_concurrent_tools: 3,
        hook_timeout_seconds: 5.0,
        trace_enabled: true,
        ..Default::default()
    };

    let agent = ReActAgent::new(
        "AsyncAgent",
        adapter,
        Some(registry),
        None,
        config,
        5,
    );
    let agent = Arc::new(Mutex::new(agent));

    let result = run_agent_async(
        agent,
        "搜索 Python 异步编程的资料，并计算 123 + 456，同时查询北京的天气",
        Some(Arc::new(on_agent_start)),
        Some(Arc::new(on_agent_finish)),
        Some(Arc::new(on_error)),
        HashMap::new(),
    )
        .await?;

    println!("最终结果: {}", result);
    Ok(())
}