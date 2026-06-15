use crate::core::types::message::Message;
/// =================================
/// 历史记录管理器
/// =================================
pub trait HistoryManager: Send + Sync {
    fn add_message(&mut self, msg: Message);
    fn messages(&self) -> Vec<Message>;
    fn clear(&mut self);
    fn estimate_tokens(&self) -> usize;
    fn compress(&mut self, summary: &str);
}
