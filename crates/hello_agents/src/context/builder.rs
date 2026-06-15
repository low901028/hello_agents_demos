// src/context/builder.rs
// GSSC 上下文构建器 (Gather-Select-Structure-Compress)

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::context::token_counter::count_tokens;
use crate::core::types::message::Message;

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
            max_tokens: 8000,
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

    pub fn build(
        &self,
        user_query: &str,
        conversation_history: Option<&[Message]>,
        system_instructions: Option<&str>,
        additional_packets: Option<Vec<ContextPacket>>,
    ) -> String {
        let packets = self.gather(
            user_query,
            conversation_history.unwrap_or(&[]),
            system_instructions,
            additional_packets.unwrap_or_default(),
        );
        let selected = self.select(packets, user_query);
        let structured = self.structure(&selected, user_query, system_instructions);
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
        if let Some(instructions) = system_instructions {
            let mut meta = HashMap::new();
            meta.insert("type".to_string(), "instructions".to_string());
            packets.push(ContextPacket::new(instructions.to_string(), meta));
        }

        if !conversation_history.is_empty() {
            let recent = if conversation_history.len() > 10 {
                &conversation_history[conversation_history.len() - 10..]
            } else {
                conversation_history
            };
            let history_text: Vec<String> = recent
                .iter()
                .map(|msg| {
                    let content = msg.content.clone().unwrap_or_default();
                    format!("[{}] {}", msg.role, content)
                })
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
        let user_query_lower = user_query.to_lowercase();
        let query_tokens: HashSet<&str> = user_query_lower.split_whitespace().collect();

        let mut scored: Vec<(f64, ContextPacket)> = packets
            .into_iter()
            .map(|mut p| {
                let p_content = p.content.to_lowercase();
                let content_tokens: HashSet<&str> = p_content.split_whitespace().collect();
                if !query_tokens.is_empty() {
                    let overlap = query_tokens.intersection(&content_tokens).count();
                    p.relevance_score = overlap as f64 / query_tokens.len() as f64;
                } else {
                    p.relevance_score = 0.0;
                }
                let now = SystemTime::now();
                let delta = now
                    .duration_since(p.timestamp)
                    .unwrap_or(Duration::ZERO)
                    .as_secs_f64();
                let tau = 3600.0;
                let rec = (-delta / tau).exp();
                let score = 0.7 * p.relevance_score + 0.3 * rec;
                (score, p)
            })
            .collect();

        let system_packets: Vec<ContextPacket> = scored
            .iter()
            .filter(|(_, p)| p.metadata.get("type").map(|s| s.as_str()) == Some("instructions"))
            .map(|(_, p)| p.clone())
            .collect();

        scored.retain(|(_, p)| p.metadata.get("type").map(|s| s.as_str()) != Some("instructions"));
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut remaining: Vec<ContextPacket> = scored.into_iter().map(|(_, p)| p).collect();
        remaining.retain(|p| p.relevance_score >= self.config.min_relevance);

        let available = self.config.get_available_tokens();
        let mut selected: Vec<ContextPacket> = Vec::new();
        let mut used_tokens = 0;

        for p in system_packets {
            if used_tokens + p.token_count <= available {
                used_tokens += p.token_count;
                selected.push(p);
            }
        }
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
        _system_instructions: Option<&str>,
    ) -> String {
        let mut sections = Vec::new();

        let p0: Vec<&ContextPacket> = selected
            .iter()
            .filter(|p| p.metadata.get("type").map(|s| s.as_str()) == Some("instructions"))
            .collect();
        if !p0.is_empty() {
            let mut role = "[Role & Policies]\n".to_string();
            role.push_str(
                &p0.iter()
                    .map(|p| p.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            sections.push(role);
        }

        sections.push(format!("[Task]\n用户问题：{}", user_query));

        let p1: Vec<&ContextPacket> = selected
            .iter()
            .filter(|p| p.metadata.get("type").map(|s| s.as_str()) == Some("task_state"))
            .collect();
        if !p1.is_empty() {
            let mut state = "[State]\n关键进展与未决问题：\n".to_string();
            state.push_str(
                &p1.iter()
                    .map(|p| p.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            sections.push(state);
        }

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

        let p3: Vec<&ContextPacket> = selected
            .iter()
            .filter(|p| p.metadata.get("type").map(|s| s.as_str()) == Some("history"))
            .collect();
        if !p3.is_empty() {
            let mut context = "[Context]\n对话历史与背景：\n".to_string();
            context.push_str(
                &p3.iter()
                    .map(|p| p.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            sections.push(context);
        }

        let output = "[Output]\n请按以下格式回答：\n1. 结论（简洁明确）\n2. 依据（列出支撑证据及来源）\n3. 风险与假设（如有）\n4. 下一步行动建议（如适用）";
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
