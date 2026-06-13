//! task_tool.rs
//! Task 工具 - 子代理调用工具，依赖 Agent trait 及子代理类型

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::{json, Value};

use crate::core::agent_trait::{Agent, SubagentResult};
use crate::core::config::Config;
use crate::core::exceptions::HelloAgentException;
use crate::tools::tool_base::{Tool, ToolBase, ToolParameter};
use crate::tools::tool_error::ToolErrorCode;
use crate::tools::tool_filter::{FullAccessFilter, ReadOnlyFilter, ToolFilter};
use crate::tools::tool_registry::ToolRegistry;
use crate::tools::tool_response::{ToolResponse, ToolStatus};

pub struct TaskTool {
    base: ToolBase,
    agent_factory: Box<dyn Fn(&str) -> Box<dyn Agent> + Send + Sync>,
    tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    config: Config,
}

impl TaskTool {
    pub fn new(
        agent_factory: impl Fn(&str) -> Box<dyn Agent> + Send + Sync + 'static,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        config: Option<Config>,
    ) -> Self {
        Self {
            base: ToolBase::new(
                "Task",
                "启动子代理处理特定的子任务，使用隔离的上下文。适用于：探索代码库、规划任务、实现功能等需要独立上下文的场景。",
                false,
            ),
            agent_factory: Box::new(agent_factory),
            tool_registry,
            config: config.unwrap_or_default(),
        }
    }

    fn create_tool_filter(&self, filter_type: &str) -> Option<Box<dyn ToolFilter>> {
        match filter_type {
            "readonly" => Some(Box::new(ReadOnlyFilter::new(None))),
            "full" => Some(Box::new(FullAccessFilter::new(None))),
            _ => None,
        }
    }
}

impl Tool for TaskTool {
    fn name(&self) -> &str { &self.base.name }
    fn base(&self) -> &ToolBase { &self.base }
    fn base_mut(&mut self) -> &mut ToolBase { &mut self.base }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("task", "string", "子任务的详细描述，告诉子代理具体要做什么", true, None),
            ToolParameter::new("agent_type", "string", "子代理类型：react/reflection/plan/simple", false, Some(json!("react"))),
            ToolParameter::new("tool_filter", "string", "工具过滤策略：readonly/full/none", false, Some(json!("none"))),
            ToolParameter::new("max_steps", "integer", "最大步数限制（覆盖默认配置）", false, None),
        ]
    }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let start = Instant::now();
        let task = parameters.get("task").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let agent_type = parameters.get("agent_type").and_then(|v| v.as_str()).unwrap_or("react").to_lowercase();
        let tool_filter_type = parameters.get("tool_filter").and_then(|v| v.as_str()).unwrap_or("none").to_lowercase();
        let max_steps = parameters.get("max_steps").and_then(|v| v.as_u64()).map(|v| v as usize);

        if task.is_empty() {
            return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "参数 'task' 不能为空", None, None));
        }

        let mut subagent = (self.agent_factory)(&agent_type);
        let tool_filter = self.create_tool_filter(&tool_filter_type);

        let result = subagent.run_as_subagent(&task, tool_filter, true, max_steps)
            .map_err(|e| HelloAgentException::llm(format!("子代理执行失败: {}", e)))?;

        let elapsed_ms = start.elapsed().as_millis() as i64;

        if result.success {
            Ok(ToolResponse::success(
                format!("[SubAgent-{}] 任务完成\n\n{}", agent_type, result.summary),
                Some(json!({
                    "agent_type": agent_type,
                    "task": task,
                    "steps": result.metadata.steps,
                    "tokens": result.metadata.tokens,
                    "duration_seconds": result.metadata.duration_seconds,
                    "tools_used": result.metadata.tools_used,
                })),
                Some(json!({"time_ms": elapsed_ms})),
                None,
            ))
        } else {
            Ok(ToolResponse::partial(
                format!("[SubAgent-{}] 任务未完全完成\n\n{}", agent_type, result.summary),
                Some(json!({
                    "agent_type": agent_type,
                    "task": task,
                    "steps": result.metadata.steps,
                    "tokens": result.metadata.tokens,
                    "duration_seconds": result.metadata.duration_seconds,
                    "tools_used": result.metadata.tools_used,
                    "error": result.metadata.error,
                })),
                Some(json!({"time_ms": elapsed_ms})),
                None,
            ))
        }
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        unimplemented!("TaskTool 不支持克隆")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::agent_trait::{Agent, SubagentMetadata, SubagentResult};
    use std::collections::HashMap;

    struct MockAgent { success: bool }

    impl Agent for MockAgent {
        fn name(&self) -> &str { "mock" }
        fn run(&mut self, task: &str, _kwargs: HashMap<String, String>) -> Result<String, HelloAgentException> {
            if self.success { Ok(format!("模拟完成: {}", task)) } else { Err(HelloAgentException::General("模拟失败".into())) }
        }
        fn build_tool_schemas(&self) -> Vec<HashMap<String, Value>> { vec![] }
        fn execute_tool_call(&self, _: &str, _: Value) -> String { String::new() }
        fn run_as_subagent(&mut self, task: &str, _: Option<Box<dyn ToolFilter>>, _: bool, _: Option<usize>) -> Result<SubagentResult, HelloAgentException> {
            let start = std::time::Instant::now();
            let result = self.run(task, HashMap::new());
            let duration = start.elapsed().as_secs_f64();
            match result {
                Ok(output) => Ok(SubagentResult {
                    success: true,
                    summary: format!("摘要: {}", output),
                    metadata: SubagentMetadata { steps: 2, tokens: 10, duration_seconds: duration, tools_used: vec!["Read".into()], error: None },
                }),
                Err(e) => Ok(SubagentResult {
                    success: false,
                    summary: format!("失败: {}", e),
                    metadata: SubagentMetadata { steps: 0, tokens: 0, duration_seconds: duration, tools_used: vec![], error: Some(e.to_string()) },
                }),
            }
        }
    }

    fn create_tool(success: bool) -> TaskTool {
        TaskTool::new(move |_| Box::new(MockAgent { success }), None, None)
    }

    #[test]
    fn test_task_tool_success() {
        let tool = create_tool(true);
        let resp = tool.run(json!({"task": "测试任务", "agent_type": "react"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
    }

    #[test]
    fn test_task_tool_partial() {
        let tool = create_tool(false);
        let resp = tool.run(json!({"task": "失败任务"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Partial);
    }

    #[test]
    fn test_missing_task_returns_error() {
        let tool = create_tool(true);
        let resp = tool.run(json!({"task": ""})).unwrap();
        assert_eq!(resp.status, ToolStatus::Error);
    }
}