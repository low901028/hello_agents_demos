use std::sync::{Arc, Mutex};
use crate::core::traits::adapter::LLMAdapter;
use crate::core::types::config::Config;
use crate::context::history::HistoryManager;
use crate::tools::registry::ToolRegistry;

/// Agent 运行上下文，提供 LLM、工具、配置、历史等依赖。
pub trait AgentContext: Send + Sync {
    fn llm(&self) -> &dyn LLMAdapter;
    fn config(&self) -> &Config;
    fn history(&self) -> &HistoryManager;
    fn history_mut(&mut self) -> &mut HistoryManager;
    fn tool_registry(&self) -> Option<&ToolRegistry>;
    fn tool_registry_arc(&self) -> Option<Arc<Mutex<ToolRegistry>>>;  // 新增
}