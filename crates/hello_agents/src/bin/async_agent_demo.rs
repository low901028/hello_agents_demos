// examples/async_agent_demo.rs

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::Future;
use serde_json::{json, Value};

use hello_agents::agents::react::ReActAgent;
use hello_agents::core::traits::agent::Agent;
use hello_agents::core::traits::lifecycle::LifecycleHook;
use hello_agents::core::traits::tool::{Tool, ToolParameter};
use hello_agents::core::types::config::Config;
use hello_agents::core::types::event::AgentEvent;
use hello_agents::core::types::exceptions::HelloAgentException;
use hello_agents::infra::llm::hello_agents_llm::HelloAgentsLLM;
use hello_agents::tools::response::{ToolResponse, ToolStatus};
use hello_agents::tools::registry::ToolRegistry;
use hello_agents::tools::tool_base::ToolBase;

// ------------------------------------------------------------
// 示例工具
// ------------------------------------------------------------

/// 搜索工具
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

/// 计算器工具
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
    let result = event.data.get("result").map(|v| v.as_str().unwrap_or("")).unwrap_or("");
    let steps = event.data.get("total_steps").and_then(|v| v.as_u64()).unwrap_or(0);
    let tokens = event.data.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    println!("\n✅ [{}] 执行完成", event.agent_name);
    println!("   总步骤: {}", steps);
    println!("   总 Token: {}", tokens);
    println!("   结果: {}", result);
}

async fn on_error(event: AgentEvent) {
    let error = event.data.get("error").map(|v| v.as_str().unwrap_or("")).unwrap_or("");
    let error_type = event.data.get("error_type").map(|v| v.as_str().unwrap_or("")).unwrap_or("");
    println!("\n❌ 错误: {} - {}", error_type, error);
}

// ------------------------------------------------------------
// 主函数
// ------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), HelloAgentException> {
    println!("{}", "=".repeat(60));
    println!("异步 Agent 生命周期示例");
    println!("{}", "=".repeat(60));

    // 加载 .env 环境变量（可选）
    dotenvy::dotenv().ok();

    // 1. 初始化 LLM（从环境变量读取）
    let llm = HelloAgentsLLM::new(Some("deepseek-v4-flash"), None, None, None, None, None, None)?;
    let adapter = llm.adapter();  // 获取 Arc<dyn LLMAdapter>

    // 2. 创建工具注册表
    let mut registry = ToolRegistry::new(None);
    registry.register_tool(Box::new(SearchTool::new()), false);
    registry.register_tool(Box::new(CalculatorTool::new()), false);
    let registry = Arc::new(Mutex::new(registry)); // 标准 std::sync::Mutex 用于 spawn_blocking

    // 3. 配置
    let config = Config {
        max_concurrent_tools: 3,
        hook_timeout_seconds: 5.0,
        trace_enabled: true,
        ..Default::default()
    };

    // 4. 创建 ReActAgent（注意构造参数需要 Arc<dyn LLMAdapter>，而不是 HelloAgentsLLM）
    let mut agent = ReActAgent::new(
        "AsyncAgent",
        adapter,
        Some(registry),
        None,              // system_prompt 使用默认
        config,
        5,                 // max_steps
    );

    // ---- 核心修改：使用 Arc<Mutex<>> + spawn_blocking 安全异步执行 ----
    let agent = Arc::new(Mutex::new(agent));
    let input = "搜索 Python 异步编程的资料，并计算 123 + 456".to_owned();
    let kwargs = HashMap::new();

    // 触发开始事件（在主异步任务中）
    let start_event = AgentEvent::new(
        hello_agents::core::types::event::EventType::AgentStart,
        "AsyncAgent".into(),
        {
            let mut data = HashMap::new();
            data.insert("input_text".into(), Value::String(input.clone()));
            data
        },
    );
    on_agent_start(start_event).await;

    // 在阻塞线程中执行同步 run
    let agent_clone = agent.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut guard = agent_clone.lock().unwrap();
        guard.run(&input, kwargs)
    })
        .await
        .map_err(|e| HelloAgentException::llm(format!("异步任务失败: {}", e)))?;

    // 处理结果和事件
    match &result {
        Ok(res) => {
            let finish_event = AgentEvent::new(
                hello_agents::core::types::event::EventType::AgentFinish,
                "AsyncAgent".into(),
                {
                    let mut data = HashMap::new();
                    data.insert("result".into(), Value::String(res.clone()));
                    data
                },
            );
            on_agent_finish(finish_event).await;
        }
        Err(e) => {
            let error_event = AgentEvent::new(
                hello_agents::core::types::event::EventType::AgentError,
                "AsyncAgent".into(),
                {
                    let mut data = HashMap::new();
                    data.insert("error".into(), Value::String(e.to_string()));
                    data.insert("error_type".into(), Value::String("HelloAgentException".into()));
                    data
                },
            );
            on_error(error_event).await;
        }
    }

    result?;
    println!("执行成功！");
    Ok(())
}