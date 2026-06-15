use crate::core::traits::expandable::ExpandableTool;
use crate::core::traits::tool::Tool;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::response::ToolResponse;
use async_trait::async_trait;
use serde_json::Value;
/// =================================
/// Tool注册中心
/// =================================
#[async_trait]
pub trait ToolRegistry: Send + Sync {
    fn list_tools(&self) -> Vec<String>;
    fn get_tool(&self, name: &str) -> Option<&dyn Tool>;
    async fn execute(&self, name: &str, args: Value) -> Result<ToolResponse, HelloAgentError>;
    fn register(&mut self, tool: Box<dyn Tool>);
    fn register_expandable(&mut self, expandable: Box<dyn ExpandableTool>);
}
