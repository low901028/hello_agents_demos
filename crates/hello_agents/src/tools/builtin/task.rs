use std::sync::{Arc, Mutex};
use std::time::Instant;
use serde_json::{json, Value};
use crate::core::traits::agent::{SubagentMetadata, SubagentResult};
use crate::core::types::config::Config;
use crate::core::types::exceptions::HelloAgentException;
use crate::tools::error::ToolErrorCode;
use crate::tools::filter::{ToolFilter, ReadOnlyFilter, FullAccessFilter};
use crate::tools::registry::ToolRegistry;
use crate::tools::response::{ToolResponse, ToolStatus};
use crate::tools::tool_base::ToolBase;
use crate::core::traits::tool::{Tool, ToolParameter};

pub trait SubagentExecutor: Send + Sync {
    fn run_as_subagent(&mut self, task: &str, tool_filter: Option<Box<dyn ToolFilter>>, return_summary: bool, max_steps_override: Option<usize>) -> Result<SubagentResult, HelloAgentException>;
}

pub struct TaskTool {
    base: ToolBase,
    agent_factory: Box<dyn Fn(&str) -> Box<dyn SubagentExecutor> + Send + Sync>,
    tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    config: Config,
}

impl TaskTool {
    pub fn new(
        agent_factory: impl Fn(&str) -> Box<dyn SubagentExecutor> + Send + Sync + 'static,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        config: Option<Config>,
    ) -> Self {
        Self {
            base: ToolBase::new("Task", "启动子代理处理特定子任务...", false),
            agent_factory: Box::new(agent_factory),
            tool_registry,
            config: config.unwrap_or_default(),
        }
    }
}

impl Tool for TaskTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str {
        &self.base.description
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("task", "string", "子任务描述", true, None),
            ToolParameter::new("agent_type", "string", "子代理类型", false, Some(json!("react"))),
            ToolParameter::new("tool_filter", "string", "工具过滤策略", false, Some(json!("none"))),
            ToolParameter::new("max_steps", "integer", "最大步数", false, None),
        ]
    }
    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let start = Instant::now();
        let task = parameters.get("task").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if task.is_empty() { return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "task 不能为空", None, None)); }
        let agent_type = parameters.get("agent_type").and_then(|v| v.as_str()).unwrap_or("react").to_lowercase();
        let filter_type = parameters.get("tool_filter").and_then(|v| v.as_str()).unwrap_or("none").to_lowercase();
        let max_steps = parameters.get("max_steps").and_then(|v| v.as_u64()).map(|v| v as usize);
        let mut sub = (self.agent_factory)(&agent_type);
        let result = sub.run_as_subagent(&task, None, true, max_steps)
            .map_err(|e| HelloAgentException::llm(format!("子代理失败: {}", e)))?;
        let elapsed = start.elapsed().as_millis() as i64;
        if result.success {
            Ok(ToolResponse::success(format!("[SubAgent-{}] 完成\n{}", agent_type, result.summary), Some(json!({"steps": result.metadata.steps, "tools": result.metadata.tools_used})), Some(json!({"time_ms": elapsed})), None))
        } else {
            Ok(ToolResponse::partial(format!("[SubAgent-{}] 未完成\n{}", agent_type, result.summary), Some(json!({"error": result.metadata.error})), Some(json!({"time_ms": elapsed})), None))
        }
    }
    fn box_clone(&self) -> Box<dyn Tool> { unimplemented!() }
}