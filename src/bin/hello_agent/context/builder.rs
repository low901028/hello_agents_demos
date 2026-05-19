use crate::hello_agent::core::message::Message;
use crate::hello_agent::context::token_counter::TokenCounter;
use chrono::{DateTime, Utc};

/// 上下文信息包
#[derive(Debug, Clone)]
pub struct ContextPacket {
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: serde_json::Value,
    pub token_count: usize,
    pub relevance_score: f64,
}

impl ContextPacket {
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        let token_count = TokenCounter::count_text(&content);
        Self {
            content,
            timestamp: Utc::now(),
            metadata: serde_json::json!({}),
            token_count,
            relevance_score: 0.0,
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// 上下文构建配置
#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub max_tokens: usize,
    pub reserve_ratio: f64,
    pub min_relevance: f64,
    pub enable_mmr: bool,
    pub mmr_lambda: f64,
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
            enable_compression: true,
        }
    }
}

impl ContextConfig {
    pub fn get_available_tokens(&self) -> usize {
        (self.max_tokens as f64 * (1.0 - self.reserve_ratio)) as usize
    }
}

/// 上下文构建器 - GSSC 流水线
pub struct ContextBuilder {
    config: ContextConfig,
}

impl ContextBuilder {
    pub fn new(config: Option<ContextConfig>) -> Self {
        Self {
            config: config.unwrap_or_default(),
        }
    }

    /// 构建完整上下文
    pub fn build(
        &self,
        user_query: &str,
        conversation_history: Option<&[Message]>,
        system_instructions: Option<&str>,
    ) -> String {
        // 1. Gather
        let packets = self.gather(user_query, conversation_history, system_instructions);

        // 2. Select
        let selected = self.select(&packets, user_query);

        // 3. Structure
        let structured = self.structure(&selected, user_query, system_instructions);

        // 4. Compress
        self.compress(&structured)
    }

    fn gather(
        &self,
        user_query: &str,
        conversation_history: Option<&[Message]>,
        system_instructions: Option<&str>,
    ) -> Vec<ContextPacket> {
        let mut packets = Vec::new();

        // P0: 系统指令
        if let Some(instructions) = system_instructions {
            packets.push(
                ContextPacket::new(instructions)
                    .with_metadata(serde_json::json!({"type": "instructions"})),
            );
        }

        // P3: 对话历史
        if let Some(history) = conversation_history {
            let recent = if history.len() > 10 {
                &history[history.len() - 10..]
            } else {
                history
            };
            let history_text: String = recent
                .iter()
                .map(|msg| format!("[{}] {}", msg.role.as_str(), msg.content))
                .collect::<Vec<_>>()
                .join("\n");

            packets.push(
                ContextPacket::new(history_text)
                    .with_metadata(serde_json::json!({"type": "history", "count": recent.len()})),
            );
        }

        packets
    }

    fn select(&self, packets: &[ContextPacket], user_query: &str) -> Vec<ContextPacket> {
        let user_query_lower = user_query.to_lowercase();
        let query_tokens: std::collections::HashSet<&str> =
            user_query_lower.split_whitespace().collect();

        let mut scored: Vec<(f64, ContextPacket)> = packets
            .iter()
            .map(|p| {
                let content_lower = p.content.to_lowercase();
                let content_tokens: std::collections::HashSet<&str> =
                    content_lower.split_whitespace().collect();
                let overlap = query_tokens.intersection(&content_tokens).count();
                let relevance = if !query_tokens.is_empty() {
                    overlap as f64 / query_tokens.len() as f64
                } else {
                    0.0
                };
                (relevance, p.clone())
            })
            .collect();

        // 系统指令固定纳入
        let mut selected = Vec::new();
        let available = self.config.get_available_tokens();
        let mut used_tokens = 0;

        for (_, p) in scored.iter().filter(|(_, p)| {
            p.metadata.get("type").and_then(|v| v.as_str()) == Some("instructions")
        }) {
            if used_tokens + p.token_count <= available {
                used_tokens += p.token_count;
                selected.push(p.clone());
            }
        }

        // 按分数加入其余
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        for (_, p) in scored {
            if p.metadata.get("type").and_then(|v| v.as_str()) == Some("instructions") {
                continue;
            }
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

        // Role & Policies
        let p0: Vec<_> = selected
            .iter()
            .filter(|p| p.metadata.get("type").and_then(|v| v.as_str()) == Some("instructions"))
            .collect();
        if !p0.is_empty() {
            let role_section = format!(
                "[Role & Policies]\n{}",
                p0.iter().map(|p| p.content.as_str()).collect::<Vec<_>>().join("\n")
            );
            sections.push(role_section);
        }

        // Task
        sections.push(format!("[Task]\n用户问题：{}", user_query));

        // Context (history)
        let p3: Vec<_> = selected
            .iter()
            .filter(|p| p.metadata.get("type").and_then(|v| v.as_str()) == Some("history"))
            .collect();
        if !p3.is_empty() {
            sections.push(format!(
                "[Context]\n{}",
                p3.iter().map(|p| p.content.as_str()).collect::<Vec<_>>().join("\n")
            ));
        }

        // Output
        sections.push(
            "[Output]\n请按以下格式回答：\n1. 结论\n2. 依据\n3. 风险与假设\n4. 下一步建议"
                .to_string(),
        );

        sections.join("\n\n")
    }

    fn compress(&self, context: &str) -> String {
        if !self.config.enable_compression {
            return context.to_string();
        }

        let current_tokens = TokenCounter::count_text(context);
        let available = self.config.get_available_tokens();

        if current_tokens <= available {
            return context.to_string();
        }

        // 简单截断
        let lines: Vec<&str> = context.lines().collect();
        let mut compressed = Vec::new();
        let mut used = 0;

        for line in lines {
            let line_tokens = TokenCounter::count_text(line);
            if used + line_tokens > available {
                break;
            }
            compressed.push(line);
            used += line_tokens;
        }

        compressed.join("\n")
    }
}