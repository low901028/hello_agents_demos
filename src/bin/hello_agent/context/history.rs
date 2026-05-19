use crate::hello_agent::core::message::Message;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// 历史消息管理器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryManager {
    history: Vec<Message>,
    pub min_retain_rounds: usize,
    pub compression_threshold: f64,
}

impl HistoryManager {
    pub fn new(min_retain_rounds: usize, compression_threshold: f64) -> Self {
        Self {
            history: Vec::new(),
            min_retain_rounds,
            compression_threshold,
        }
    }

    pub fn append(&mut self, message: Message) {
        self.history.push(message);
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

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// 预估完整轮次数
    pub fn estimate_rounds(&self) -> usize {
        let mut rounds = 0;
        let mut i = 0;
        while i < self.history.len() {
            if self.history[i].role.as_str() == "user" {
                rounds += 1;
                i += 1;
                while i < self.history.len() && self.history[i].role.as_str() != "user" {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        rounds
    }

    /// 查找每轮的起始索引
    pub fn find_round_boundaries(&self) -> Vec<usize> {
        self.history
            .iter()
            .enumerate()
            .filter(|(_, msg)| msg.role.as_str() == "user")
            .map(|(i, _)| i)
            .collect()
    }

    /// 压缩历史
    pub fn compress(&mut self, summary: &str) {
        let rounds = self.estimate_rounds();
        if rounds <= self.min_retain_rounds {
            return;
        }

        let boundaries = self.find_round_boundaries();
        if boundaries.len() <= self.min_retain_rounds {
            return;
        }

        let keep_from_index = boundaries[boundaries.len() - self.min_retain_rounds];

        let summary_msg = Message::new_with_metadata(
            format!("## Archived Session Summary\n{}", summary),
            crate::hello_agent::core::message::MessageRole::Summary,
            serde_json::json!({"compressed_at": Utc::now().to_rfc3339()}),
        );

        self.history = {
            let mut new_history = vec![summary_msg];
            new_history.extend_from_slice(&self.history[keep_from_index..]);
            new_history
        };
    }

    /// 序列化
    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::json!({
            "history": self.history.iter().map(|m| m.to_dict()).collect::<Vec<_>>(),
            "created_at": Utc::now().to_rfc3339(),
            "rounds": self.estimate_rounds(),
        })
    }

    /// 从字典加载
    pub fn load_from_dict(&mut self, data: &serde_json::Value) {
        if let Some(history) = data.get("history").and_then(|v| v.as_array()) {
            self.history = history
                .iter()
                .filter_map(|m| Message::from_dict(m))
                .collect();
        }
    }
}