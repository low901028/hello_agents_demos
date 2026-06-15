//! examples/axum_sse_server.rs
//! 测试方式：
//!     curl -N http://localhost:8000/agent/stream -X POST -H "Content-Type: application/json" -d '{"input": "你好"}'
//!     curl -N http://localhost:8000/agent/stream -X POST -H "Content-Type: application/json" -d '{"input": "1+2+3"}'
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use std::pin::Pin;
use tower_http::cors::{CorsLayer, Any};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use hello_agents::agents::plan_solve::PlanSolveAgent;
use hello_agents::agents::react::ReActAgent;
use hello_agents::agents::reflection::ReflectionAgent;
use hello_agents::agents::simple::SimpleAgent;
use hello_agents::context::history_manager_impl::HistoryManagerImpl;
use hello_agents::core::agent_runtime::AgentRuntime;
use hello_agents::core::traits::agent::Agent;
use hello_agents::core::traits::history::HistoryManager;
use hello_agents::core::traits::llm_provider::LlmProvider;
use hello_agents::core::traits::tool::Tool;
use hello_agents::core::traits::tool_registry::ToolRegistry;
use hello_agents::core::types::config::Config;
use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::core::types::response::ToolResponse;
use hello_agents::infra::openai_adapter::OpenAIAdapter;
use hello_agents::infra::tool_registry_impl::ToolRegistryImpl;
// ==================== 共享状态 ====================
#[derive(Clone)]
struct AppState {
    llm: Arc<OpenAIAdapter>,
    tools: Arc<ToolRegistryImpl>,
    config: Config,
}

// ==================== 计算器工具 ====================

struct CalculatorTool;

#[async_trait::async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str { "Calculator" }
    fn description(&self) -> &str { "执行数学计算，支持加减乘除" }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": { "type": "string", "description": "数学表达式" }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResponse, HelloAgentError> {
        let expr = args.get("expression").or(args.get("input"))
            .and_then(|v| v.as_str()).unwrap_or("");
        match evalexpr::eval(expr) {
            Ok(result) => Ok(ToolResponse::success(format!("计算结果: {}", result))),
            Err(e) => Ok(ToolResponse::error("CALC_ERROR", &format!("计算错误: {}", e))),
        }
    }
}

// ==================== 请求模型 ====================

#[derive(Deserialize)]
struct AgentRequest {
    input: String,
    #[serde(default = "default_agent_type")]
    agent_type: String,
}

fn default_agent_type() -> String { "react".into() }

// ==================== 路由处理 ====================
async fn agent_stream(
    State(state): State<AppState>,
    Json(req): Json<AgentRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let agent_type = req.agent_type;
    let input = req.input;

    // 创建运行时，所有 Agent 共享
    let history = Arc::new(std::sync::Mutex::new(Box::new(HistoryManagerImpl::new(10, 0.8)) as Box<dyn HistoryManager>));
    let runtime = AgentRuntime::new(
        state.llm.clone() as Arc<dyn hello_agents::core::traits::llm_provider::LlmProvider>,
        state.tools.clone() as Arc<dyn hello_agents::core::traits::tool_registry::ToolRegistry>,
        history,
        state.config.clone(),
    );

    // 唯一的流生成器，内部处理所有分支
    let stream = async_stream::stream! {
        // 根据 agent_type 构造 agent（如果无效，直接发送错误事件并返回）
        let mut agent: Option<Box<dyn Agent>> = match agent_type.as_str() {
            "react" => Some(Box::new(ReActAgent::new("ReActAssistant", None, 5))),
            "simple" => Some(Box::new(SimpleAgent::new("SimpleAssistant", Some("你是一个有用的助手".into()), 5))),
            "reflection" => Some(Box::new(ReflectionAgent::new("ReflectionAssistant", Some("你是一个反思型助手".into()), 2))),
            "plan" => Some(Box::new(PlanSolveAgent::new(
                "PlanAssistant",
                None,
            ))),
            _ => None,
        };

        match agent {
            None => {
                // 无效的 agent_type，发送错误事件
                yield Ok(Event::default().data("{\"error\":\"未知的 agent_type\"}"));
            }
            Some(ref mut agent) => {
                // 发送开始事件
                yield Ok(Event::default().data("{\"type\":\"agent_start\"}"));

                // 执行 Agent 并将结果切块模拟流式
                match agent.run(&input, &runtime).await {
                    Ok(result) => {
                        for chunk in result.chars().collect::<Vec<_>>().chunks(10) {
                            let text: String = chunk.iter().collect();
                            yield Ok(Event::default().data(format!("{{\"type\":\"chunk\", \"text\":\"{}\"}}", text)));
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                        yield Ok(Event::default().data("{\"type\":\"agent_finish\"}"));
                    }
                    Err(e) => {
                        yield Ok(Event::default().data(format!("{{\"type\":\"error\", \"message\":\"{}\"}}", e)));
                    }
                }
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive-text"),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let api_key = std::env::var("LLM_API_KEY").expect("LLM_API_KEY not set");
    let base_url = std::env::var("LLM_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("LLM_MODEL_ID").unwrap_or_else(|_| "deepseek-chat".into());
    let llm = Arc::new(OpenAIAdapter::new(&api_key, &base_url, &model));

    let mut tools = ToolRegistryImpl::new();
    tools.register(Box::new(CalculatorTool));
    let tools = Arc::new(tools);

    let config = Config {
        stream_enabled: true,
        ..Default::default()
    };

    let state = AppState { llm, tools, config };

    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);
    let app = Router::new()
        .route("/", get(root))
        .route("/agent/stream", post(agent_stream))
        .layer(cors)
        .with_state(state);

    println!("🚀 服务器启动于 http://0.0.0.0:8000");
    // ---- 使用 axum::Server::bind 兼容旧版本 ----
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "message": "HelloAgents SSE Demo",
        "endpoints": {
            "stream": "/agent/stream",
        }
    }))
}