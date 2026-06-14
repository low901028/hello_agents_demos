use std::collections::HashMap;
use crate::core::types::message::{Message, MessageContent};

pub fn count_tokens(text: &str) -> usize { text.chars().count() / 4 }

#[derive(Clone)]
pub struct TokenCounter { pub model: String, cache: HashMap<String, usize> }
impl TokenCounter {
    pub fn new(model: &str) -> Self { Self { model: model.to_string(), cache: HashMap::new() } }
    pub fn count_message(&mut self, msg: &Message) -> usize { /* 同前 */ 0 }
    pub fn count_messages(&mut self, msgs: &[Message]) -> usize { msgs.iter().map(|m| self.count_message(m)).sum() }
    pub fn clear_cache(&mut self) { self.cache.clear(); }

    /// 获取缓存统计信息
    pub fn get_cache_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        stats.insert("cached_messages".to_string(), self.cache.len());
        stats.insert(
            "total_cached_tokens".to_string(),
            self.cache.values().sum(),
        );
        stats
    }
}