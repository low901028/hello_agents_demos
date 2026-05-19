use crate::hello_agent::core::message::Message;
use std::collections::HashMap;

/// Token 计数器（简化版，使用字符估算）
pub struct TokenCounter {
    model: String,
    cache: HashMap<String, usize>,
}

impl TokenCounter {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            cache: HashMap::new(),
        }
    }

    /// 计算消息列表的 Token 数
    pub fn count_messages(&self, messages: &[Message]) -> usize {
        messages.iter().map(|m| self.count_message(m)).sum()
    }

    /// 计算单条消息的 Token 数（带缓存）
    pub fn count_message(&self, message: &Message) -> usize {
        let cache_key = format!("{}:{}", message.role.as_str(), message.content);
        if let Some(&tokens) = self.cache.get(&cache_key) {
            return tokens;
        }

        let tokens = Self::count_text(&message.content) + 4; // +4 for role overhead
        // Note: cache is immutable in this method, caller should use a mutable version if needed
        tokens
    }

    /// 计算文本 Token 数
    pub fn count_text(text: &str) -> usize {
        // 降级方案：1 token ≈ 4 字符
        text.len() / 4
    }

    /// 清空缓存
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// 获取缓存大小
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}