/// =================================
/// Tool过滤器
/// =================================
pub trait ToolFilter: Send + Sync {
    fn filter(&self, all_tools: &[String]) -> Vec<String>;
    fn is_allowed(&self, tool_name: &str) -> bool;
}
