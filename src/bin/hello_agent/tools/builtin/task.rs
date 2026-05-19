use std::collections::HashMap;
use std::sync::Arc;

use crate::hello_agent::core::agent::Agent;
use crate::hello_agent::core::config::Config;
use crate::hello_agent::core::llm::HelloAgentsLLM;
use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::filter::ToolFilter;
use crate::hello_agent::tools::registry::ToolRegistry;
use crate::hello_agent::tools::response::ToolResponse;

/// 子代理工具
pub struct TaskTool {
    agent_factory: Arc<dyn Fn(&str) -> Option<Box<dyn Agent>> + Send + Sync>,
    tool_registry: Option<Arc<std::sync::Mutex<ToolRegistry>>>,
    config: Config,
}

impl TaskTool {
    pub fn new(
        agent_factory: Arc<dyn Fn(&str) -> Option<Box<dyn Agent>> + Send + Sync>,
        tool_registry: Option<Arc<std::sync::Mutex<ToolRegistry>>>,
        config: Option<Config>,
    ) -> Self {
        Self {
            agent_factory,
            tool_registry,
            config: config.unwrap_or_default(),
        }
    }
}

impl Tool for TaskTool {
    fn name(&self) -> &str { "Task" }

    fn description(&self) -> &str {
        "启动子代理处理特定的子任务，使用隔离的上下文。适用于探索代码库、规划任务、实现功能等。"
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("task", "string", "子任务的详细描述", true),
            ToolParameter::new("agent_type", "string", "子代理类型：react|reflection|plan|simple", false)
                .with_default(serde_json::json!("react")),
            ToolParameter::new("tool_filter", "string", "工具过滤策略：readonly|full|none", false)
                .with_default(serde_json::json!("none")),
            ToolParameter::new("max_steps", "integer", "最大步数限制", false),
        ]
    }

    fn run(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        let task = parameters.get("task").and_then(|v| v.as_str()).unwrap_or("");
        let agent_type = parameters.get("agent_type").and_then(|v| v.as_str()).unwrap_or("react");

        if task.is_empty() {
            return ToolResponse::error(ToolErrorCode::INVALID_PARAM, "参数 'task' 不能为空");
        }

        // 尝试创建子代理
        match (self.agent_factory)(agent_type) {
            Some(_agent) => {
                // 在实际使用中，这里会执行子代理并返回结果
                ToolResponse::success(format!(
                    "[SubAgent-{}] 任务已接收: {}...",
                    agent_type,
                    &task[..task.len().min(50)]
                ))
                    .with_data("agent_type", agent_type)
            }
            None => {
                ToolResponse::error(
                    ToolErrorCode::INVALID_PARAM,
                    format!("不支持的 agent_type: {}", agent_type),
                )
            }
        }
    }
}