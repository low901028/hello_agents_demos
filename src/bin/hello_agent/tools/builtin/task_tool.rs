use crate::hello_agent::core::agent::Agent;
use crate::hello_agent::core::config::Config;
use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::ToolResponse;
use std::collections::HashMap;
use std::sync::Arc;

pub struct TaskTool {
    agent_factory: Arc<dyn Fn(&str) -> Box<dyn Agent> + Send + Sync>,
    tool_registry: Option<Arc<crate::hello_agent::tools::registry::ToolRegistry>>,
    config: Config,
}

impl TaskTool {
    pub fn new(
        agent_factory: Arc<dyn Fn(&str) -> Box<dyn Agent> + Send + Sync>,
        tool_registry: Option<Arc<crate::hello_agent::tools::registry::ToolRegistry>>,
        config: Option<Config>,
    ) -> Self {
        TaskTool {
            agent_factory,
            tool_registry,
            config: config.unwrap_or_default(),
        }
    }
}

impl Tool for TaskTool {
    fn name(&self) -> &str {
        "Task"
    }
    fn description(&self) -> &str {
        "启动子代理处理特定的子任务，使用隔离的上下文"
    }
    fn run(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        let task = parameters
            .get("task")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if task.is_empty() {
            return ToolResponse::error("INVALID_PARAM", "参数'task'不能为空");
        }
        let agent_type = parameters
            .get("agent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("react");
        let mut subagent = (self.agent_factory)(agent_type);
        let result = subagent.run_as_subagent(task, None, true, None);
        let success = result
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let summary = result
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if success {
            ToolResponse::success(
                format!("[SubAgent-{}] 任务完成\n\n{}", agent_type, summary),
                result,
            )
        } else {
            ToolResponse::partial(
                format!("[SubAgent-{}] 任务未完全完成\n\n{}", agent_type, summary),
                result,
            )
        }
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("task", "string", "子任务的详细描述"),
            ToolParameter::optional(
                "agent_type",
                "string",
                "子代理类型：react/reflection/plan/simple",
            )
            .with_default(serde_json::json!("react")),
        ]
    }
}
