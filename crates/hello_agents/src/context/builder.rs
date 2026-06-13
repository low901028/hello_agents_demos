//! context_builder.rs
//! GSSC 上下文构建器 (Gather-Select-Structure-Compress)
//! ContextBuilder - GSSC流水线实现
//!
//! 实现 Gather-Select-Structure-Compress 上下文构建流程：
//! 1. Gather: 从多源收集候选信息（历史、工具结果）
//! 2. Select: 基于优先级、相关性、多样性筛选
//! 3. Structure: 组织成结构化上下文模板
//! 4. Compress: 在预算内压缩与规范化

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::core::llm_resp_req::Message;

/// 计算 token 数（简化实现）
fn count_tokens(text: &str) -> usize {
    // 注意：原 Python 使用 tiktoken，这里简化为 1 token ≈ 4 字符
    text.chars().count() / 4
}

/// 上下文信息包
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPacket {
    pub content: String,
    pub timestamp: SystemTime,
    pub metadata: HashMap<String, String>,
    pub token_count: usize,
    pub relevance_score: f64,
}

impl ContextPacket {
    pub fn new(content: String, metadata: HashMap<String, String>) -> Self {
        let token_count = count_tokens(&content);
        Self {
            content,
            timestamp: SystemTime::now(),
            metadata,
            token_count,
            relevance_score: 0.0,
        }
    }
}

/// 上下文构建配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    pub max_tokens: usize,
    pub reserve_ratio: f64,
    pub min_relevance: f64,
    pub enable_mmr: bool,
    pub mmr_lambda: f64,
    pub system_prompt_template: String,
    pub enable_compression: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 128_000,
            reserve_ratio: 0.15,
            min_relevance: 0.3,
            enable_mmr: true,
            mmr_lambda: 0.7,
            system_prompt_template: String::new(),
            enable_compression: true,
        }
    }
}

impl ContextConfig {
    // 获取可用token预算（扣除余量）
    pub fn get_available_tokens(&self) -> usize {
        (self.max_tokens as f64 * (1.0 - self.reserve_ratio)) as usize
    }
}

/// 上下文构建器
pub struct ContextBuilder {
    pub config: ContextConfig,
}

impl ContextBuilder {
    pub fn new(config: Option<ContextConfig>) -> Self {
        Self {
            config: config.unwrap_or_default(),
        }
    }

    /// 构建完整上下文
    ///
    ///         Args:
    ///             user_query: 用户查询
    ///             conversation_history: 对话历史
    ///             system_instructions: 系统指令
    ///             additional_packets: 额外的上下文包
    ///
    ///         Returns:
    ///             结构化上下文字符串
    pub fn build(
        &self,
        user_query: &str,
        conversation_history: Option<&[Message]>,
        system_instructions: Option<&str>,
        additional_packets: Option<Vec<ContextPacket>>,
    ) -> String {
        // 1. Gather
        let packets = self.gather(
            user_query,
            conversation_history.unwrap_or(&[]),
            system_instructions,
            additional_packets.unwrap_or_default(),
        );

        // 2. Select
        let selected = self.select(packets, user_query);

        // 3. Structure
        let structured = self.structure(&selected, user_query, system_instructions);

        // 4. Compress
        self.compress(&structured)
    }

    fn gather(
        &self,
        _user_query: &str,
        conversation_history: &[Message],
        system_instructions: Option<&str>,
        additional_packets: Vec<ContextPacket>,
    ) -> Vec<ContextPacket> {
        let mut packets = Vec::new();

        // P0: 系统指令
        if let Some(instructions) = system_instructions {
            let mut meta = HashMap::new();
            meta.insert("type".to_string(), "instructions".to_string());
            packets.push(ContextPacket::new(instructions.to_string(), meta));
        }

        // P3: 对话历史
        if !conversation_history.is_empty() {
            let recent = if conversation_history.len() > 10 {
                &conversation_history[conversation_history.len() - 10..]
            } else {
                conversation_history
            };
            let history_text: Vec<String> = recent
                .iter()
                .map(|msg| format!("[{:?}] {:?}", msg.role.as_str(), msg.content))
                .collect();
            let mut meta = HashMap::new();
            meta.insert("type".to_string(), "history".to_string());
            meta.insert("count".to_string(), recent.len().to_string());
            packets.push(ContextPacket::new(history_text.join("\n"), meta));
        }

        packets.extend(additional_packets);
        packets
    }

    fn select(&self, packets: Vec<ContextPacket>, user_query: &str) -> Vec<ContextPacket> {
        let user_query = user_query.to_lowercase();
        // 1) 计算相关性分数
        let query_tokens: HashSet<&str> = user_query.split_whitespace().collect();
        let mut scored: Vec<(f64, ContextPacket)> = packets
            .into_iter()
            .map(|mut p| {
                let content =  p.content.to_lowercase();
                let content_tokens: HashSet<&str> = content.split_whitespace().collect();
                if !query_tokens.is_empty() {
                    let overlap = query_tokens.intersection(&content_tokens).count();
                    p.relevance_score = overlap as f64 / query_tokens.len() as f64;
                } else {
                    p.relevance_score = 0.0;
                }

                // 2) 新近性得分
                let now = SystemTime::now();
                let delta = now.duration_since(p.timestamp).unwrap_or(Duration::ZERO).as_secs_f64();
                let tau = 3600.0; // 1小时
                let rec = (-delta / tau).exp();

                let score = 0.7 * p.relevance_score + 0.3 * rec;
                (score, p)
            })
            .collect();

        // 3) 分离系统指令
        let system_packets: Vec<ContextPacket> = scored
            .iter()
            .filter(|(_, p)| p.metadata.get("type").map(|s| s.as_str()) == Some("instructions"))
            .map(|(_, p)| p.clone())
            .collect();

        // 4) 剩余包按分数排序
        scored.retain(|(_, p)| p.metadata.get("type").map(|s| s.as_str()) != Some("instructions"));
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut remaining: Vec<ContextPacket> = scored.into_iter().map(|(_, p)| p).collect();

        // 5) 相关性过滤
        remaining.retain(|p| p.relevance_score >= self.config.min_relevance);

        // 6) 按预算填充
        let available = self.config.get_available_tokens();
        let mut selected: Vec<ContextPacket> = Vec::new();
        let mut used_tokens = 0;

        // 先放系统指令
        for p in system_packets {
            if used_tokens + p.token_count <= available {
                used_tokens += p.token_count;
                selected.push(p);
            }
        }

        // 再按分数加入其余
        for p in remaining {
            if used_tokens + p.token_count > available {
                continue;
            }
            used_tokens += p.token_count;
            selected.push(p);
        }

        selected
    }

    fn structure(
        &self,
        selected: &[ContextPacket],
        user_query: &str,
        system_instructions: Option<&str>,
    ) -> String {
        let mut sections = Vec::new();

        // [Role & Policies]
        let p0: Vec<&ContextPacket> = selected
            .iter()
            .filter(|p| p.metadata.get("type").map(|s| s.as_str()) == Some("instructions"))
            .collect();
        if !p0.is_empty() {
            let mut role = "[Role & Policies]\n".to_string();
            role.push_str(&p0.iter().map(|p| p.content.as_str()).collect::<Vec<_>>().join("\n"));
            sections.push(role);
        }

        // [Task]
        sections.push(format!("[Task]\n用户问题：{}", user_query));

        // [State]
        let p1: Vec<&ContextPacket> = selected
            .iter()
            .filter(|p| p.metadata.get("type").map(|s| s.as_str()) == Some("task_state"))
            .collect();
        if !p1.is_empty() {
            let mut state = "[State]\n关键进展与未决问题：\n".to_string();
            state.push_str(&p1.iter().map(|p| p.content.as_str()).collect::<Vec<_>>().join("\n"));
            sections.push(state);
        }

        // [Evidence]
        let p2: Vec<&ContextPacket> = selected
            .iter()
            .filter(|p| {
                let t = p.metadata.get("type").map(|s| s.as_str());
                t == Some("related_memory")
                    || t == Some("knowledge_base")
                    || t == Some("retrieval")
                    || t == Some("tool_result")
            })
            .collect();
        if !p2.is_empty() {
            let mut evidence = "[Evidence]\n事实与引用：\n".to_string();
            for p in &p2 {
                evidence.push_str(&format!("\n{}\n", p.content));
            }
            sections.push(evidence);
        }

        // [Context] 历史等
        let p3: Vec<&ContextPacket> = selected
            .iter()
            .filter(|p| p.metadata.get("type").map(|s| s.as_str()) == Some("history"))
            .collect();
        if !p3.is_empty() {
            let mut context = "[Context]\n对话历史与背景：\n".to_string();
            context.push_str(&p3.iter().map(|p| p.content.as_str()).collect::<Vec<_>>().join("\n"));
            sections.push(context);
        }

        // [Output]
        let output = "[Output]\n\
                    请按以下格式回答：\n\
                    1. 结论（简洁明确）\n\
                    2. 依据（列出支撑证据及来源）\n\
                    3. 风险与假设（如有）\n\
                    4. 下一步行动建议（如适用）";
        sections.push(output.to_string());

        sections.join("\n\n")
    }

    fn compress(&self, context: &str) -> String {
        if !self.config.enable_compression {
            return context.to_string();
        }

        let current_tokens = count_tokens(context);
        let available = self.config.get_available_tokens();

        if current_tokens <= available {
            return context.to_string();
        }

        println!(
            "⚠️ 上下文超预算 ({} > {})，执行截断",
            current_tokens, available
        );

        // 按段落截断
        let lines: Vec<&str> = context.lines().collect();
        let mut compressed = Vec::new();
        let mut used = 0;

        for line in lines {
            let lt = count_tokens(line);
            if used + lt > available {
                break;
            }
            compressed.push(line);
            used += lt;
        }

        compressed.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::llm_resp_req::{Message, MessageContent};
    #[cfg(unix)]
    use std::time::UNIX_EPOCH;

    fn create_message(role: &str, content: &str, timestamp: SystemTime) -> Message {
        Message {
            role: crate::core::llm_resp_req::MessageRole::from_str(role).unwrap(),
            content: Some(MessageContent::Text(content.to_string())),
            tool_call_id: None,
            tool_calls: None,
            name: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_builder_basic() {
        let config = ContextConfig::default();
        let builder = ContextBuilder::new(Some(config));

        let history = vec![
            create_message("user", "今天天气怎么样", SystemTime::now()),
            create_message("assistant", "天气很好", SystemTime::now() - Duration::from_secs(1800)),
        ];

        let result = builder.build(
            "今天天气怎么样",
            Some(&history),
            Some("你是一个友好的助手"),
            None,
        );

        assert!(result.contains("[Role & Policies]"));
        assert!(result.contains("你是一个友好的助手"));
        assert!(result.contains("[Task]"));
        assert!(result.contains("[Context]"));
        assert!(result.contains("[Output]"));
    }

    #[test]
    fn test_compress_truncation() {
        let config = ContextConfig {
            max_tokens: 100,
            reserve_ratio: 0.0,
            enable_compression: true,
            ..Default::default()
        };
        let builder = ContextBuilder::new(Some(config));
        let long_text = "a".repeat(1000);
        // compress 应该只返回部分内容
        let compressed = builder.compress(&long_text);
        assert!(compressed.len() < long_text.len());
    }
}