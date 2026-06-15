use crate::core::agent_runtime::AgentRuntime;
use crate::core::traits::tool_filter::ToolFilter;
use crate::core::types::exceptions::HelloAgentError;
use async_trait::async_trait;

#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    async fn run(&mut self, input: &str, runtime: &AgentRuntime)
        -> Result<String, HelloAgentError>;
    async fn run_as_subagent(
        &mut self,
        task: &str,
        parent_runtime: &AgentRuntime,
        filter: Option<&dyn ToolFilter>,
        max_steps: Option<usize>,
    ) -> Result<String, HelloAgentError> {
        self.run(task, parent_runtime).await
    }
}
