use crate::hello_agent::core::message::{Message, MessageRole};
use chrono::Utc;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct HistoryManager {
    history: Vec<Message>,
    min_retain_rounds: usize,
    compression_threshold: f64,
}

impl HistoryManager {
    pub fn new(min_retain_rounds: usize, compression_threshold: f64) -> Self {
        HistoryManager {
            history: Vec::new(),
            min_retain_rounds,
            compression_threshold,
        }
    }
    pub fn append(&mut self, msg: Message) {
        self.history.push(msg);
    }
    pub fn get_history(&self) -> Vec<Message> {
        self.history.clone()
    }
    pub fn clear(&mut self) {
        self.history.clear();
    }
    pub fn len(&self) -> usize {
        self.history.len()
    }
    pub fn estimate_rounds(&self) -> usize {
        let mut rounds = 0;
        let mut i = 0;
        while i < self.history.len() {
            if self.history[i].role == MessageRole::User {
                rounds += 1;
                i += 1;
                while i < self.history.len() && self.history[i].role != MessageRole::User {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        rounds
    }
    pub fn find_round_boundaries(&self) -> Vec<usize> {
        self.history
            .iter()
            .enumerate()
            .filter(|(_, m)| m.role == MessageRole::User)
            .map(|(i, _)| i)
            .collect()
    }
    pub fn compress(&mut self, summary: &str) {
        let rounds = self.estimate_rounds();
        if rounds <= self.min_retain_rounds {
            return;
        }
        let boundaries = self.find_round_boundaries();
        if boundaries.len() > self.min_retain_rounds {
            let keep_from = boundaries[boundaries.len() - self.min_retain_rounds];
            let summary_msg = Message {
                content: format!("## Archived Summary\n{}", summary),
                role: MessageRole::Summary,
                timestamp: Utc::now(),
                metadata: HashMap::new(),
            };
            let kept = self.history.split_off(keep_from);
            self.history = vec![summary_msg];
            self.history.extend(kept);
        }
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        HistoryManager::new(10, 0.8)
    }
}
