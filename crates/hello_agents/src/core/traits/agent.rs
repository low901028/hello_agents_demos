use std::collections::HashMap;
use std::pin::Pin;

use async_trait::async_trait;
use futures::stream::Stream;

use crate::core::traits::context::AgentContext;
use crate::core::types::event::StreamEvent;
use crate::core::types::exceptions::HelloAgentException;
use crate::tools::filter::ToolFilter;

/// 子代理元数据
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

#[async_trait]
pub trait Agent {
    type Context: AgentContext;

    fn name(&self) -> &str;
    fn context(&self) -> &Self::Context;
    fn context_mut(&mut self) -> &mut Self::Context;

    /// 同步执行（核心方法）
    fn run(
        &mut self,
        input_text: &str,
        kwargs: HashMap<String, String>,
    ) -> Result<String, HelloAgentException>;

    /// 构建工具 JSON Schema 列表
    fn build_tool_schemas(&self) -> Vec<HashMap<String, serde_json::Value>>;

    /// 执行工具调用并返回字符串结果
    fn execute_tool_call(&self, tool_name: &str, arguments: serde_json::Value) -> String;

    /// 作为子代理运行（上下文隔离）
    fn run_as_subagent(
        &mut self,
        task: &str,
        tool_filter: Option<Box<dyn ToolFilter>>,
        return_summary: bool,
        max_steps_override: Option<usize>,
    ) -> Result<SubagentResult, HelloAgentException>;

    // 异步生命周期方法（提供默认实现）
    async fn arun(
        &mut self,
        input_text: &str,
        on_start: Option<crate::core::traits::lifecycle::LifecycleHook>,
        on_finish: Option<crate::core::traits::lifecycle::LifecycleHook>,
        on_error: Option<crate::core::traits::lifecycle::LifecycleHook>,
        kwargs: HashMap<String, String>,
    ) -> Result<String, HelloAgentException> {
        self.run(input_text, kwargs)
    }

    async fn arun_stream(
        &mut self,
        _input_text: &str,
        _kwargs: HashMap<String, String>,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        unimplemented!()
    }
}