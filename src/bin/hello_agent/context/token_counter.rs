use crate::hello_agent::core::message::Message;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TokenCounter {
    model: String,
    cache: HashMap<String, usize>,
}

impl TokenCounter {
    pub fn new(model: &str) -> Self {
        TokenCounter {
            model: model.into(),
            cache: HashMap::new(),
        }
    }
    pub fn count_messages(&mut self, msgs: &[Message]) -> usize {
        msgs.iter().map(|m| self.count_message(m)).sum()
    }
    pub fn count_message(&mut self, msg: &Message) -> usize {
        let key = format!("{}:{}", msg.role.as_str(), msg.content);
        if let Some(&t) = self.cache.get(&key) {
            return t;
        }
        let tokens = self.estimate(&msg.content) + 4;
        self.cache.insert(key, tokens);
        tokens
    }
    fn estimate(&self, text: &str) -> usize {
        let cjk = text.chars().filter(|c| matches!(c, '\u{4E00}'..='\u{9FFF}'|'\u{3400}'..='\u{4DBF}'|'\u{3000}'..='\u{303F}'|'\u{3040}'..='\u{30FF}')).count();
        (cjk as f64 * 1.5 + (text.chars().count() - cjk) as f64 / 4.0).ceil() as usize
    }
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}
