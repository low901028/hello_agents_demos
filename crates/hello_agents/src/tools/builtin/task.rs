use crate::core::traits::tool::Tool;
use crate::core::traits::tool_filter::ToolFilter;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::response::ToolResponse;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

// SubagentExecutor trait 定义
pub trait SubagentExecutor: Send + Sync {
    fn run_as_subagent(
        &mut self,
        task: &str,
        filter: Option<&dyn ToolFilter>,
        return_summary: bool,
        max_steps: Option<usize>,
    ) -> Result<String, HelloAgentError>;
}

pub struct TaskTool {
    agent_factory: Arc<dyn Fn(&str) -> Box<dyn SubagentExecutor> + Send + Sync>,
}

impl TaskTool {
    pub fn new(
        factory: impl Fn(&str) -> Box<dyn SubagentExecutor> + Send + Sync + 'static,
    ) -> Self {
        Self {
            agent_factory: Arc::new(factory),
        }
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "Task"
    }
    fn description(&self) -> &str {
        "启动子代理处理特定的子任务"
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task": { "type": "string", "description": "子任务描述" },
                "agent_type": { "type": "string", "description": "子代理类型", "default": "react" },
                "tool_filter": { "type": "string", "description": "工具过滤策略", "default": "none" },
                "max_steps": { "type": "integer", "description": "最大步数" }
            },
            "required": ["task"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let task = args["task"].as_str().unwrap_or("");
        let agent_type = args["agent_type"].as_str().unwrap_or("react");
        let max_steps = args["max_steps"].as_u64().map(|n| n as usize);
        if task.is_empty() {
            return Ok(ToolResponse::error("INVALID_PARAM", "任务描述不能为空"));
        }
        let mut sub = (self.agent_factory)(agent_type);
        let result = sub.run_as_subagent(task, None, true, max_steps)?;
        Ok(ToolResponse::success(result))
    }
}
