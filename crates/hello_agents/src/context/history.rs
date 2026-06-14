//! src/context/history.rs
//! 历史管理器 - 对应 Python HistoryManager

use std::collections::HashMap;

use chrono::Utc;
use serde_json::Value;

use crate::core::types::message::{Message, MessageContent, MessageRole};

/// 历史管理器
///
/// 特性：
/// - 只追加，不编辑（缓存友好）
/// - 自动压缩历史（summary + 保留最近轮次）
/// - 支持会话保存/加载
#[derive(Clone)]
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
    ///
    /// # Arguments
    /// * `min_retain_rounds` - 压缩时保留的最小完整轮次数
    /// * `compression_threshold` - 压缩阈值（预留，暂未使用）
    pub fn new(min_retain_rounds: usize, compression_threshold: f64) -> Self {
        Self {
            history: Vec::new(),
            min_retain_rounds,
            compression_threshold,
        }
    }

    /// 追加消息（只追加，不编辑）
    ///
    /// # Arguments
    /// * `message` - 要追加的消息
    pub fn append(&mut self, message: Message) {
        self.history.push(message);
    }

    /// 获取历史副本
    ///
    /// # Returns
    /// 历史消息列表的副本
    pub fn get_history(&self) -> Vec<Message> {
        self.history.clone()
    }

    /// 清空历史
    pub fn clear(&mut self) {
        self.history.clear();
    }

    /// 预估完整轮次数
    ///
    /// 一轮定义：1 user 消息 + N 条 assistant/tool/summary 消息
    ///
    /// # Returns
    /// 完整轮次数
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
    ///
    /// # Returns
    /// 每轮起始索引列表，例如 [0, 3, 7, 10]
    pub fn find_round_boundaries(&self) -> Vec<usize> {
        self.history
            .iter()
            .enumerate()
            .filter(|(_, msg)| msg.role == MessageRole::User)
            .map(|(i, _)| i)
            .collect()
    }

    /// 压缩历史：将旧历史替换为 summary 消息，保留最近 N 轮完整对话
    ///
    /// # Arguments
    /// * `summary` - 历史摘要文本
    pub fn compress(&mut self, summary: &str) {
        let rounds = self.estimate_rounds();
        if rounds <= self.min_retain_rounds {
            return;
        }

        let boundaries = self.find_round_boundaries();
        if boundaries.len() <= self.min_retain_rounds {
            return;
        }

        // 保留最近 min_retain_rounds 轮
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
    ///
    /// # Returns
    /// 包含历史和元数据的字典
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
    ///
    /// # Arguments
    /// * `data` - 序列化的历史数据
    pub fn load_from_dict(&mut self, data: &HashMap<String, Value>) {
        if let Some(history_arr) = data.get("history").and_then(|v| v.as_array()) {
            self.history = history_arr
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
        }
    }
}