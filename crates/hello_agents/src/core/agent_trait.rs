//! agent_trait.rs
//! Agent 抽象接口，供 task_tool 等模块使用，避免循环依赖

use std::collections::HashMap;
use std::pin::Pin;

use async_trait::async_trait;
use futures::stream::Stream;
use serde_json::Value;
use crate::core::exceptions::HelloAgentException;
use crate::core::lifecycle::{AgentEvent, LifecycleHook};
use crate::tools::tool_filter::ToolFilter;
use crate::tools::tool_response::{ToolResponse, ToolStatus};


/// 子代理执行元数据
#[derive(Debug, Clone)]
pub struct SubagentMetadata {
    pub steps: usize,
    pub tokens: usize,
    pub duration_seconds: f64,
    pub tools_used: Vec<String>,
    pub error: Option<String>,
}

/// 子代理执行结果
#[derive(Debug, Clone)]
pub struct SubagentResult {
    pub success: bool,
    pub summary: String,
    pub metadata: SubagentMetadata,
}

/// Agent 抽象接口
#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;

    /// 同步执行（核心方法）
    fn run(&mut self, input_text: &str, kwargs: HashMap<String, String>) -> Result<String, HelloAgentException>;

    /// 构建工具 JSON Schema 列表
    fn build_tool_schemas(&self) -> Vec<HashMap<String, Value>>;

    /// 执行工具调用并返回字符串结果
    fn execute_tool_call(&self, tool_name: &str, arguments: Value) -> String;

    /// 作为子代理运行（上下文隔离），供 TaskTool 调用（同步方法）
    fn run_as_subagent(
        &mut self,
        task: &str,
        tool_filter: Option<Box<dyn ToolFilter>>,
        return_summary: bool,
        max_steps_override: Option<usize>,
    ) -> Result<SubagentResult, HelloAgentException>;

    // 异步生命周期方法（可选）
    async fn arun(
        &mut self,
        input_text: &str,
        on_start: Option<LifecycleHook>,
        on_finish: Option<LifecycleHook>,
        on_error: Option<LifecycleHook>,
        kwargs: HashMap<String, String>,
    ) -> Result<String, HelloAgentException> {
        self.run(input_text, kwargs)
    }

    async fn arun_stream(
        &mut self,
        _input_text: &str,
        _kwargs: HashMap<String, String>,
    ) -> Pin<Box<dyn Stream<Item = AgentEvent> + Send>> {
        todo!()
    }
}
