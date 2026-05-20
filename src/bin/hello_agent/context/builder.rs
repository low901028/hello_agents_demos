use crate::hello_agent::core::message::Message;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ContextPacket {
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub token_count: usize,
    pub relevance_score: f64,
}

impl ContextPacket {
    pub fn new(content: impl Into<String>, metadata: HashMap<String, serde_json::Value>) -> Self {
        let c = content.into();
        ContextPacket {
            token_count: c.len() / 4,
            content: c,
            timestamp: Utc::now(),
            metadata,
            relevance_score: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub max_tokens: usize,
    pub reserve_ratio: f64,
    pub min_relevance: f64,
    pub enable_compression: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        ContextConfig {
            max_tokens: 8000,
            reserve_ratio: 0.15,
            min_relevance: 0.3,
            enable_compression: true,
        }
    }
}
impl ContextConfig {
    pub fn available_tokens(&self) -> usize {
        (self.max_tokens as f64 * (1.0 - self.reserve_ratio)) as usize
    }
}

pub struct ContextBuilder {
    config: ContextConfig,
}

impl ContextBuilder {
    pub fn new(config: Option<ContextConfig>) -> Self {
        ContextBuilder {
            config: config.unwrap_or_default(),
        }
    }
    pub fn build(
        &self,
        query: &str,
        history: Option<&[Message]>,
        system: Option<&str>,
        extra: Option<Vec<ContextPacket>>,
    ) -> String {
        let packets = self.gather(history, system, extra);
        let selected = self.select(packets, query);
        let structured = self.structure(&selected, query);
        self.compress(&structured)
    }
    fn gather(
        &self,
        history: Option<&[Message]>,
        system: Option<&str>,
        extra: Option<Vec<ContextPacket>>,
    ) -> Vec<ContextPacket> {
        let mut packets = Vec::new();
        if let Some(sys) = system {
            let mut meta = HashMap::new();
            meta.insert("type".into(), serde_json::json!("instructions"));
            packets.push(ContextPacket::new(sys, meta));
        }
        if let Some(hist) = history {
            let recent = if hist.len() > 10 {
                &hist[hist.len() - 10..]
            } else {
                hist
            };
            let text: String = recent
                .iter()
                .map(|m| format!("[{}]{}", m.role.as_str(), m.content))
                .collect::<Vec<_>>()
                .join("\n");
            let mut meta = HashMap::new();
            meta.insert("type".into(), serde_json::json!("history"));
            packets.push(ContextPacket::new(text, meta));
        }
        if let Some(e) = extra {
            packets.extend(e);
        }
        packets
    }
    fn select(&self, packets: Vec<ContextPacket>, query: &str) -> Vec<ContextPacket> {
        let available = self.config.available_tokens();
        let mut scored: Vec<(f64, ContextPacket)> = packets
            .into_iter()
            .map(|mut p| {
                p.relevance_score = Self::relevance(&p.content, query);
                let r = Self::recency(p.timestamp);
                (0.7 * p.relevance_score + 0.3 * r, p)
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut selected = Vec::new();
        let mut used = 0;
        for (_, p) in scored {
            if p.metadata.get("type").and_then(|v| v.as_str()) == Some("instructions") {
                if used + p.token_count <= available {
                    used += p.token_count;
                    selected.push(p);
                }
            } else if p.relevance_score >= self.config.min_relevance
                && used + p.token_count <= available
            {
                used += p.token_count;
                selected.push(p);
            }
        }
        selected
    }
    fn relevance(content: &str, query: &str) -> f64 {
        let qt: HashSet<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();

        if qt.is_empty() {
            return 0.0;
        }

        let ct: HashSet<String> = content
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();

        qt.intersection(&ct).count() as f64 / qt.len() as f64
    }
    fn recency(ts: DateTime<Utc>) -> f64 {
        (-(Utc::now() - ts).num_seconds().max(0) as f64 / 3600.0).exp()
    }
    fn structure(&self, packets: &[ContextPacket], query: &str) -> String {
        let mut sections = Vec::new();
        let inst: Vec<&str> = packets
            .iter()
            .filter(|p| p.metadata.get("type").and_then(|v| v.as_str()) == Some("instructions"))
            .map(|p| p.content.as_str())
            .collect();
        if !inst.is_empty() {
            sections.push(format!("[Role & Policies]\n{}", inst.join("\n")));
        }
        sections.push(format!("[Task]\n用户问题：{}", query));
        let hist: Vec<&str> = packets
            .iter()
            .filter(|p| p.metadata.get("type").and_then(|v| v.as_str()) == Some("history"))
            .map(|p| p.content.as_str())
            .collect();
        if !hist.is_empty() {
            sections.push(format!("[Context]\n{}", hist.join("\n")));
        }
        sections.push("[Output]\n1.结论\n2.依据\n3.风险\n4.下一步".into());
        sections.join("\n\n")
    }
    fn compress(&self, ctx: &str) -> String {
        if !self.config.enable_compression || ctx.len() / 4 <= self.config.available_tokens() {
            return ctx.into();
        }
        let mut lines = Vec::new();
        let mut used = 0;
        for l in ctx.lines() {
            let t = l.len() / 4;
            if used + t > self.config.available_tokens() {
                break;
            }
            lines.push(l);
            used += t;
        }
        lines.join("\n")
    }
}
