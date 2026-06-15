// examples/async_agent_demo.rs

use evalexpr::{ContextWithMutableVariables, HashMapContext};
use futures::Future;
use hello_agents::agents::react::ReActAgent;
use hello_agents::context::history_manager_impl::HistoryManagerImpl;
use hello_agents::core::agent_runtime::AgentRuntime;
use hello_agents::core::traits::agent::Agent;
use hello_agents::core::traits::history::HistoryManager;
use hello_agents::core::traits::lifecycle::LifecycleHook;
use hello_agents::core::traits::llm_provider;
use hello_agents::core::traits::tool::Tool;
use hello_agents::core::traits::tool_registry::ToolRegistry;
use hello_agents::core::types::config::Config;
use hello_agents::core::types::event::{AgentEvent, EventType};
use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::core::types::response::ToolResponse;
use hello_agents::infra::openai_adapter::OpenAIAdapter;
use hello_agents::infra::tool_registry_impl::ToolRegistryImpl;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
// ------------------------------------------------------------
// 示例工具（异步版本）
// ------------------------------------------------------------

/// 搜索工具
struct SearchTool;

#[async_trait::async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &str {
        "Search"
    }
    fn description(&self) -> &str {
        "搜索互联网信息"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "搜索关键词" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        Ok(ToolResponse::success_with(
            format!("搜索结果：关于 '{}' 的信息...", query),
            Some(json!({"query": query, "results": 10})),
            None,
            None,
        ))
    }
}

/// 计算器工具（异步执行，内部使用同步 evalexpr）
struct CalculatorTool;

#[async_trait::async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "Calculator"
    }
    fn description(&self) -> &str {
        "执行数学计算"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "expression": { "type": "string", "description": "数学表达式" }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let expr = args
            .get("expression")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();

        let expr_for_response = expr.clone();

        let result = tokio::task::spawn_blocking(move || {
            let preprocessed = expr.replace("**", "^");
            let mut ctx: HashMapContext = evalexpr::HashMapContext::new();
            let _ = ctx.set_value("pi".into(), evalexpr::Value::Float(std::f64::consts::PI));
            let _ = ctx.set_value("e".into(), evalexpr::Value::Float(std::f64::consts::E));
            evalexpr::eval_with_context(&preprocessed, &ctx)
        })
        .await
        .map_err(|e| HelloAgentError::Tool(Box::new(e)))?;

        let num = result.unwrap().as_float().unwrap_or(0.0);
        let result_str = num.to_string();
        Ok(ToolResponse::success_with(
            format!("计算结果：{} = {}", expr_for_response, result_str),
            Some(json!({"expression": expr_for_response, "result": num})),
            None,
            None,
        ))
    }
}

// ------------------------------------------------------------
// 生命周期钩子
// ------------------------------------------------------------

fn make_hook<F>(f: F) -> LifecycleHook
where
    F: Fn(AgentEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync + 'static,
{
    Arc::new(f)
}

async fn on_agent_start(event: AgentEvent) {
    println!("\n🚀 [{}] 开始执行", event.agent_name);
    if let Some(input) = event.data.get("input_text") {
        println!("   输入: {}", input.as_str().unwrap_or(""));
    }
}

async fn on_agent_finish(event: AgentEvent) {
    let result = event
        .data
        .get("result")
        .map(|v| v.as_str().unwrap_or(""))
        .unwrap_or("");
    let steps = event
        .data
        .get("total_steps")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let tokens = event
        .data
        .get("total_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    println!("\n✅ [{}] 执行完成", event.agent_name);
    println!("   总步骤: {}", steps);
    println!("   总 Token: {}", tokens);
    println!("   结果: {}", result);
}

async fn on_error(event: AgentEvent) {
    let error = event
        .data
        .get("error")
        .map(|v| v.as_str().unwrap_or(""))
        .unwrap_or("");
    let error_type = event
        .data
        .get("error_type")
        .map(|v| v.as_str().unwrap_or(""))
        .unwrap_or("");
    println!("\n❌ 错误: {} - {}", error_type, error);
}

// ------------------------------------------------------------
// 主函数
// ------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let model = std::env::var("LLM_MODEL_ID").unwrap_or_else(|_| "deepseek-v4-flash".to_string());
    let api_key = std::env::var("LLM_API_KEY").expect("LLM_API_KEY not set");
    let base_url =
        std::env::var("LLM_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".to_string());

    let llm = Arc::new(OpenAIAdapter::new(&api_key, &base_url, &model));
    let mut tools = ToolRegistryImpl::new();
    tools.register(Box::new(CalculatorTool));
    tools.register(Box::new(SearchTool));
    let tools = Arc::new(tools);
    let history = Arc::new(std::sync::Mutex::new(
        Box::new(HistoryManagerImpl::new(10, 0.8)) as Box<dyn HistoryManager>,
    ));
    let config = Config::default();
    let runtime = AgentRuntime::new(llm, tools, history, config);

    let mut agent = ReActAgent::new("AsyncAgent", Some("你是一个有用的AI助手".into()), 5);
    let input = "搜索 Python 异步编程的资料，并计算 123 + 456";

    // 开始事件
    on_agent_start(AgentEvent::new(
        EventType::AgentStart,
        "AsyncAgent".into(),
        {
            let mut data = HashMap::new();
            data.insert("input_text".into(), Value::String(input.into()));
            data
        },
    ))
    .await;

    let result = agent.run(input, &runtime).await;

    match &result {
        Ok(answer) => {
            on_agent_finish(AgentEvent::new(
                EventType::AgentFinish,
                "AsyncAgent".into(),
                {
                    let mut data = HashMap::new();
                    data.insert("result".into(), Value::String(answer.clone()));
                    data.insert("total_steps".into(), Value::Number(agent.steps.into()));
                    data.insert("total_tokens".into(), Value::Number(agent.tokens.into()));
                    data
                },
            ))
            .await;
        }
        Err(e) => {
            on_error(AgentEvent::new(
                EventType::AgentError,
                "AsyncAgent".into(),
                {
                    let mut data = HashMap::new();
                    data.insert("error".into(), Value::String(e.to_string()));
                    data
                },
            ))
            .await;
        }
    }

    result?;
    println!("执行成功！");
    Ok(())
}
