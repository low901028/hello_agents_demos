use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::response::ToolResponse;
use async_trait::async_trait;
use serde_json::Value;

/// =================================
/// Tool接口
/// =================================
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError>;
    // 需要展开时 必须重写该方法
    fn is_expandable(&self) -> bool {
        false
    }
}
