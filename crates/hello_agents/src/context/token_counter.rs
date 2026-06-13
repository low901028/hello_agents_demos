//! token_counter.rs
//! Token 计数器 - TokenCounter - Token 计数器
//!
//! 职责：
//! - 本地预估 Token 数（无需 API 调用）
//! - 缓存机制（避免重复计算）
//! - 增量计算（只计算新增消息）
//! - 降级方案（tiktoken 不可用时使用字符估算）

use std::collections::HashMap;

use crate::core::llm_resp_req::{Message, MessageContent, MessageRole};

/// Token 计数器
///
/// 特性：
/// - 本地预估（无需 API 调用）
/// - 缓存机制（避免重复计算）
/// - 增量计算（只计算新增消息）
/// - 降级方案（tiktoken 不可用时使用字符估算）
///
/// 目前 Rust 实现直接使用字符估算（1 token ≈ 4 字符），因为未引入 tiktoken。
/// 未来可集成 tiktoken-rs 实现精确计算。
pub struct TokenCounter {
    /// 模型名称（预留，暂未使用）
    pub model: String,
    /// 消息内容缓存：键为 "role:content"，值为 token 数
    cache: HashMap<String, usize>,
}

impl TokenCounter {
    /// 创建新的 Token 计数器
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            cache: HashMap::new(),
        }
    }

    /// 计算消息列表的 Token 数（带缓存，增量计算）
    pub fn count_messages(&mut self, messages: &[Message]) -> usize {
        messages.iter().map(|msg| self.count_message(msg)).sum()
    }

    /// 计算单条消息的 Token 数（带缓存）
    pub fn count_message(&mut self, message: &Message) -> usize {
        // 提取 content 文本
        let content_text = extract_text(message);

        // 缓存键
        let cache_key = format!("{}:{}", message.role.as_str(), content_text);

        if let Some(&tokens) = self.cache.get(&cache_key) {
            return tokens;
        }

        // 计算 Token 数
        let mut tokens = self._count_text(&content_text);

        // 添加角色标记的开销（约 4 tokens）
        tokens += 4;

        self.cache.insert(cache_key, tokens);
        tokens
    }

    /// 计算文本的 Token 数（无缓存）
    pub fn count_text(&self, text: &str) -> usize {
        self._count_text(text)
    }

    /// 内部 Token 计算方法
    /// 降级方案：粗略估算（1 token ≈ 4 字符）
    fn _count_text(&self, text: &str) -> usize {
        // 如果集成 tiktoken-rs，可在此处替换为精确计算
        text.chars().count() / 4
    }

    /// 清空缓存
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// 获取缓存大小（缓存的消息数量）
    pub fn get_cache_size(&self) -> usize {
        self.cache.len()
    }

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

/// 从 Message 中提取纯文本内容，若不可用返回空字符串
fn extract_text(msg: &Message) -> String {
    match &msg.content {
        Some(MessageContent::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

// =============================================================================
// 测试用例
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::llm_resp_req::{Message, MessageContent, MessageRole};

    #[test]
    fn test_count_message() {
        let mut counter = TokenCounter::new("gpt-4");
        let msg = Message::new_text("Hello, world!", MessageRole::User);
        let tokens = counter.count_message(&msg);
        // "Hello, world!" 有 13 字符，/4 = 3，加4 = 7
        assert_eq!(tokens, 7);
    }

    #[test]
    fn test_cache_hit() {
        let mut counter = TokenCounter::new("gpt-4");
        let msg = Message::new_text("Hi", MessageRole::User);
        let first = counter.count_message(&msg);
        let second = counter.count_message(&msg);
        assert_eq!(first, second);
        assert_eq!(counter.get_cache_size(), 1);
    }

    #[test]
    fn test_count_messages() {
        let mut counter = TokenCounter::new("gpt-4");
        let msgs = vec![
            Message::new_text("one", MessageRole::User),
            Message::new_text("two", MessageRole::Assistant),
        ];
        let total = counter.count_messages(&msgs);
        // one: 3/4=0 +4 =4 ; two: 3/4=0+4=4 ; total=8
        assert_eq!(total, 8);
        assert_eq!(counter.get_cache_size(), 2);
    }

    #[test]
    fn test_clear_cache() {
        let mut counter = TokenCounter::new("gpt-4");
        counter.count_message(&Message::new_text( "test", MessageRole::User));
        assert_eq!(counter.get_cache_size(), 1);
        counter.clear_cache();
        assert_eq!(counter.get_cache_size(), 0);
    }

    #[test]
    fn test_get_cache_stats() {
        let mut counter = TokenCounter::new("gpt-4");
        counter.count_message(&Message::new_text("hello",MessageRole::User, ));
        counter.count_message(&Message::new_text("world",MessageRole::Assistant, ));
        let stats = counter.get_cache_stats();
        assert_eq!(stats["cached_messages"], 2);
        assert!(stats["total_cached_tokens"] > 0);
    }
}