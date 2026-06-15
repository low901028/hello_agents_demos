use crate::core::traits::tool::Tool;
/// =================================
/// 是否展开子工具
/// =================================
pub trait ExpandableTool: Tool + Send + Sync {
    fn expand(&self) -> Vec<Box<dyn Tool>>;
}
