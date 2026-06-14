// examples/simple_tool_template.rs

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use anyhow::{Result, Context};

use hello_agents::agents::react::ReActAgent;
use hello_agents::core::traits::agent::Agent;
use hello_agents::core::traits::tool::{Tool, ToolParameter};
use hello_agents::core::types::config::Config;
use hello_agents::core::types::exceptions::HelloAgentException;
use hello_agents::infra::llm::hello_agents_llm::HelloAgentsLLM;
use hello_agents::tools::error::ToolErrorCode;
use hello_agents::tools::registry::ToolRegistry;
use hello_agents::tools::response::{ToolResponse, ToolStatus};
use hello_agents::tools::tool_base::ToolBase;

// ------------------------------------------------------------
// 简单工具模板
// ------------------------------------------------------------

/// 简单工具模板，演示最基本的工具实现。
///
/// 功能：
/// - 接收输入文本
/// - 将其转换为大写
/// - 返回标准化的响应
pub struct SimpleToolTemplate {
    base: ToolBase,
}

impl SimpleToolTemplate {
    pub fn new() -> Self {
        Self {
            base: ToolBase::new(
                "simple_tool",
                "这是一个简单的工具模板，用于演示基本用法",
                false,
            ),
        }
    }

    /// 处理输入的核心逻辑（私有方法，保持 run 简洁）
    fn process_input(&self, user_input: &str) -> String {
        user_input.to_uppercase()
    }
}

impl Tool for SimpleToolTemplate {
    fn name(&self) -> &str {
        &self.base.name
    }

    fn description(&self) -> &str {
        &self.base.description
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new(
                "input",
                "string",
                "要处理的输入文本",
                true,
                None,
            ),
            // 可按需添加更多参数
            // ToolParameter::new(
            //     "option",
            //     "string",
            //     "可选参数",
            //     false,
            //     Some(Value::String("default_value".into())),
            // ),
        ]
    }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        // 1. 获取参数
        let user_input = parameters
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 2. 参数验证
        if user_input.is_empty() {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "参数 'input' 不能为空",
                None,
                Some(json!({"provided_params": parameters})),
            ));
        }

        // 3. 执行业务逻辑
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.process_input(user_input)
        })) {
            Ok(result) => {
                // 4. 返回成功响应
                Ok(ToolResponse::success(
                    format!("处理成功: {}", result),
                    Some(json!({
                        "input": user_input,
                        "output": result,
                        "processed": true,
                    })),
                    None,
                    None,
                ))
            }
            Err(e) => {
                // 5. 错误处理
                let msg = if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "未知错误".to_string()
                };
                Ok(ToolResponse::error(
                    ToolErrorCode::ExecutionError.as_str(),
                    &format!("工具执行失败: {}", msg),
                    None,
                    Some(json!({"input": user_input})),
                ))
            }
        }
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
        })
    }
}

// ------------------------------------------------------------
// 使用示例
// ------------------------------------------------------------
fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    // 1. 创建 LLM 客户端
    let llm = HelloAgentsLLM::new(Some("deepseek-v4-flash"), None, None, None, None, None, None)?;
    let adapter = llm.adapter();

    // 2. 创建工具注册表并注册工具
    let mut registry = ToolRegistry::new(None);
    let tool = Box::new(SimpleToolTemplate::new());
    registry.register_tool(tool, false);
    let registry = Arc::new(Mutex::new(registry));

    // 3. 配置
    let config = Config {
        trace_enabled: false,
        ..Default::default()
    };

    // 4. 创建 Agent
    let agent = ReActAgent::new(
        "Assistant",
        adapter,
        Some(registry),
        None,
        config,
        5,
    );
    let agent = Arc::new(Mutex::new(agent));

    // 5. 运行任务
    let result = run_agent_sync(&agent, "使用 simple_tool 处理文本 'test message'")?;
    println!("最终结果: {}", result);

    Ok(())
}

/// 同步运行 Agent（包装在 Arc<Mutex<>> 中）
fn run_agent_sync<A: Agent>(
    agent: &Arc<Mutex<A>>,
    input: &str,
) -> Result<String, HelloAgentException> {
    let mut guard = agent.lock().unwrap();
    guard.run(input, HashMap::new())
}