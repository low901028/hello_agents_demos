// src/context/history_manager_impl.rs
// 历史管理器实现，保留原有完整逻辑，适配新 Message 结构

use crate::core::traits::history::HistoryManager;
use crate::core::types::message::Message;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;

pub struct HistoryManagerImpl {
    messages: Vec<Message>,
    pub min_retain_rounds: usize,
    pub compression_threshold: f64,
}

impl HistoryManagerImpl {
    pub fn new(min_retain_rounds: usize, compression_threshold: f64) -> Self {
        Self {
            messages: Vec::new(),
            min_retain_rounds,
            compression_threshold,
        }
    }

    /// 预估完整轮次数（1 user 消息 + N 条后续消息）
    pub fn estimate_rounds(&self) -> usize {
        let mut rounds = 0;
        let mut i = 0;
        while i < self.messages.len() {
            if self.messages[i].role == "user" {
                rounds += 1;
                i += 1;
                while i < self.messages.len() && self.messages[i].role != "user" {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        rounds
    }

    /// 查找每轮的起始索引（即 user 消息的位置）
    pub fn find_round_boundaries(&self) -> Vec<usize> {
        self.messages
            .iter()
            .enumerate()
            .filter(|(_, msg)| msg.role == "user")
            .map(|(i, _)| i)
            .collect()
    }

    /// 压缩历史：将旧历史替换为 summary 消息，保留最近 N 轮完整对话
    pub fn compress_with_rounds(&mut self, summary: &str) {
        let rounds = self.estimate_rounds();
        if rounds <= self.min_retain_rounds {
            return;
        }

        let boundaries = self.find_round_boundaries();
        if boundaries.len() <= self.min_retain_rounds {
            return;
        }

        // 保留最近 min_retain_rounds 轮，其余压缩
        let keep_from = boundaries[boundaries.len() - self.min_retain_rounds];

        // 生成 summary 消息（包含压缩时间）
        let summary_content = format!(
            "## Archived Session Summary\n{}\n\n(compressed at {})",
            summary,
            Utc::now().to_rfc3339()
        );
        let summary_msg = Message::system(&summary_content);

        // 替换历史
        let tail = self.messages.split_off(keep_from);
        self.messages.clear();
        self.messages.push(summary_msg);
        self.messages.extend(tail);
    }

    /// 序列化为字典（用于会话保存）
    pub fn to_dict(&self) -> HashMap<String, Value> {
        let mut dict = HashMap::new();
        dict.insert(
            "history".to_string(),
            Value::Array(
                self.messages
                    .iter()
                    .map(|m| serde_json::to_value(m).unwrap_or_default())
                    .collect(),
            ),
        );
        dict.insert(
            "created_at".to_string(),
            Value::String(Utc::now().to_rfc3339()),
        );
        dict.insert(
            "rounds".to_string(),
            Value::Number(serde_json::Number::from(self.estimate_rounds())),
        );
        dict
    }

    /// 从字典加载（用于会话恢复）
    pub fn load_from_dict(&mut self, data: &HashMap<String, Value>) {
        if let Some(history_arr) = data.get("history").and_then(|v| v.as_array()) {
            self.messages = history_arr
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
        }
    }
}

impl HistoryManager for HistoryManagerImpl {
    fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    fn messages(&self) -> Vec<Message> {
        self.messages.clone()
    }

    fn clear(&mut self) {
        self.messages.clear();
    }

    fn estimate_tokens(&self) -> usize {
        self.messages
            .iter()
            .map(|m| m.content.as_ref().map(|c| c.len()).unwrap_or(0) / 4)
            .sum()
    }

    fn compress(&mut self, summary: &str) {
        // 使用原有的完整压缩逻辑
        self.compress_with_rounds(summary);
    }
}
