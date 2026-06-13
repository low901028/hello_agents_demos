//! history_manager.rs
//! 历史消息管理器 - 对应 Python HistoryManager，适配当前 Message 结构
//! HistoryManager - 历史消息管理器
//!
//! 职责：
//! - 消息追加（只追加，不编辑，缓存友好）
//! - 历史压缩（生成 summary + 保留最近轮次）
//! - 会话序列化/反序列化
//! - 轮次边界检测

use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::llm_resp_req::{Message, MessageContent, MessageRole};

/// 历史管理器
///
/// 特性：
/// - 只追加，不编辑（缓存友好）
/// - 自动压缩历史（summary + 保留最近轮次）
/// - 支持会话保存/加载
pub struct HistoryManager {
    /// 历史消息列表（只追加）
    history: Vec<Message>,
    /// 压缩时保留的最小完整轮次数
    pub min_retain_rounds: usize,
    /// 压缩阈值（暂未使用，预留）
    pub compression_threshold: f64,
}

impl HistoryManager {
    /// 创建新的历史管理器
    pub fn new(min_retain_rounds: usize, compression_threshold: f64) -> Self {
        Self {
            history: Vec::new(),
            min_retain_rounds,
            compression_threshold,
        }
    }

    /// 追加消息（只追加，不编辑）
    pub fn append(&mut self, message: Message) {
        self.history.push(message);
    }

    /// 获取历史副本
    pub fn get_history(&self) -> Vec<Message> {
        self.history.clone()
    }

    /// 清空历史
    pub fn clear(&mut self) {
        self.history.clear();
    }

    /// 预估完整轮次数
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

    /// 查找每轮的起始索引
    pub fn find_round_boundaries(&self) -> Vec<usize> {
        self.history
            .iter()
            .enumerate()
            .filter(|(_, msg)| msg.role == MessageRole::User)
            .map(|(i, _)| i)
            .collect()
    }

    /// 压缩历史：将旧历史替换为 summary 消息，保留最近 N 轮完整对话
    pub fn compress(&mut self, summary: &str) {
        let rounds = self.estimate_rounds();
        if rounds <= self.min_retain_rounds {
            return;
        }

        let boundaries = self.find_round_boundaries();
        if boundaries.len() <= self.min_retain_rounds {
            return;
        }

        let keep_from = boundaries[boundaries.len() - self.min_retain_rounds];

        // 生成 summary 消息
        let mut summary_msg = Message {
            role: MessageRole::Summary,
            content: Some(MessageContent::Text(format!(
                "## Archived Session Summary\n{}",
                summary
            ))),
            ..Default::default()
        };
        // 在 extra 中记录压缩时间
        let mut extra = HashMap::new();
        extra.insert(
            "compressed_at".to_string(),
            Value::String(Utc::now().to_rfc3339()),
        );
        summary_msg.extra = extra;

        // 替换历史
        let tail = self.history.split_off(keep_from);
        self.history.clear();
        self.history.push(summary_msg);
        self.history.extend(tail);
    }

    /// 序列化为字典（用于会话保存）
    pub fn to_dict(&self) -> HashMap<String, Value> {
        let mut dict = HashMap::new();
        dict.insert(
            "history".to_string(),
            Value::Array(
                self.history
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
            self.history = history_arr
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
        }
    }
}

// =============================================================================
// 测试用例
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::llm_resp_req::{Message, MessageRole};

    #[test]
    fn test_append_and_get() {
        let mut manager = HistoryManager::new(10, 0.8);
        let msg = Message::new_text("hello", MessageRole::User);
        manager.append(msg);
        assert_eq!(manager.get_history().len(), 1);
    }

    #[test]
    fn test_clear() {
        let mut manager = HistoryManager::new(10, 0.8);
        manager.append(Message::new_text("hello", MessageRole::User));
        manager.clear();
        assert!(manager.get_history().is_empty());
    }

    #[test]
    fn test_estimate_rounds() {
        let mut manager = HistoryManager::new(10, 0.8);
        manager.append(Message::new_text("u1", MessageRole::User));
        manager.append(Message::new_text("a1", MessageRole::Assistant));
        manager.append(Message::new_text("u2", MessageRole::User));
        manager.append(Message::new_text("tool call", MessageRole::Tool));
        manager.append(Message::new_text("a2", MessageRole::Assistant));
        assert_eq!(manager.estimate_rounds(), 2);
    }

    #[test]
    fn test_compress() {
        let mut manager = HistoryManager::new(2, 0.8); // 保留最近2轮
        manager.append(Message::new_text("u1", MessageRole::User));
        manager.append(Message::new_text("a1", MessageRole::Assistant));
        manager.append(Message::new_text("u2", MessageRole::User));
        manager.append(Message::new_text("a2", MessageRole::Assistant));
        manager.append(Message::new_text("u3", MessageRole::User));
        manager.append(Message::new_text("a3", MessageRole::Assistant));

        assert_eq!(manager.estimate_rounds(), 3);
        manager.compress("summary of first round");

        let history = manager.get_history();
        assert_eq!(history.len(), 5);
        assert_eq!(history[0].role, MessageRole::Summary);
        // 检查压缩时间
        assert!(history[0].extra.contains_key("compressed_at"));
    }

    #[test]
    fn test_no_compress_when_rounds_insufficient() {
        let mut manager = HistoryManager::new(3, 0.8);
        manager.append(Message::new_text("u1", MessageRole::User));
        manager.append(Message::new_text("a1", MessageRole::Assistant));
        manager.compress("summary");
        assert_eq!(manager.get_history().len(), 2);
    }

    #[test]
    fn test_to_dict_and_load() {
        let mut manager = HistoryManager::new(10, 0.8);
        manager.append(Message::new_text("hello", MessageRole::User));
        let dict = manager.to_dict();
        assert!(dict.contains_key("history"));

        let mut new_manager = HistoryManager::new(10, 0.8);
        new_manager.load_from_dict(&dict);
        assert_eq!(new_manager.get_history().len(), 1);
    }
}