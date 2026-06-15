use crate::core::types::message::Message;
use std::collections::HashMap;

pub fn count_tokens(text: &str) -> usize {
    text.chars().count() / 4
}

#[derive(Clone)]
pub struct TokenCounter {
    pub model: String,
    cache: HashMap<String, usize>,
}
impl TokenCounter {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            cache: HashMap::new(),
        }
    }
    pub fn count_message(&mut self, message: &Message) -> usize {
        let text = message.content.clone().unwrap_or_default();
        let key = format!("{}:{}", message.role, text);   // 直接使用 role 字符串
        if let Some(&tokens) = self.cache.get(&key) {
            return tokens;
        }
        let mut tokens = count_tokens(&text);
        tokens += 4; // 角色开销
        self.cache.insert(key, tokens);
        tokens
    }

    pub fn count_tokens(text: &str) -> usize {
        let estimate = text.chars().count() / 4;
        if estimate == 0 && !text.is_empty() { 1 } else { estimate }
    }

    pub fn count_messages(&mut self, msgs: &[Message]) -> usize {
        msgs.iter().map(|m| self.count_message(m)).sum()
    }
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// 获取缓存统计信息
    pub fn get_cache_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        stats.insert("cached_messages".to_string(), self.cache.len());
        stats.insert("total_cached_tokens".to_string(), self.cache.values().sum());
        stats
    }
}
