// tools/builtin/devlog.rs
// 开发日志工具 - 异步版本，直接实现 Tool trait

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::core::traits::tool::Tool;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::response::ToolResponse;
use crate::tools::error::ToolErrorCode;

// ---------- UTF‑8 安全截断 ----------
fn safe_truncate(s: &str, max_chars: usize) -> &str {
    if s.chars().count() <= max_chars {
        return s;
    }
    let mut end = 0;
    for (i, c) in s.char_indices().take(max_chars) {
        end = i + c.len_utf8();
    }
    &s[..end]
}

// ---------- 日志类别 ----------
lazy_static! {
    static ref CATEGORIES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("decision", "架构/技术选型决策");
        m.insert("progress", "阶段性进展记录");
        m.insert("issue", "遇到的问题");
        m.insert("solution", "问题解决方案");
        m.insert("refactor", "重构决策");
        m.insert("test", "测试相关记录");
        m.insert("performance", "性能优化记录");
        m
    };
}

fn categories_str() -> String {
    CATEGORIES
        .iter()
        .map(|(k, v)| format!("- {}: {}", k, v))
        .collect::<Vec<_>>()
        .join("\n")
}

fn category_keys() -> Vec<String> {
    CATEGORIES.keys().map(|s| s.to_string()).collect()
}

// ---------- 单条日志 ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevLogEntry {
    pub id: String,
    pub timestamp: String,
    pub category: String,
    pub content: String,
    #[serde(default)]
    pub metadata: HashMap<String, JsonValue>,
}

impl DevLogEntry {
    pub fn create(
        category: impl Into<String>,
        content: impl Into<String>,
        metadata: Option<HashMap<String, JsonValue>>,
    ) -> Self {
        let category = category.into();
        let content = content.into();
        Self {
            id: format!("log-{}", Uuid::new_v4().to_string().split('-').next().unwrap_or("0")),
            timestamp: Utc::now().to_rfc3339(),
            category,
            content,
            metadata: metadata.unwrap_or_default(),
        }
    }

    pub fn to_dict(&self) -> JsonValue {
        serde_json::to_value(self).unwrap_or_default()
    }

    pub fn from_dict(data: &JsonValue) -> Result<Self, serde_json::Error> {
        serde_json::from_value(data.clone())
    }
}

// ---------- 日志存储引擎 ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevLogStore {
    pub session_id: String,
    pub agent_name: String,
    pub created_at: String,
    pub updated_at: String,
    pub entries: Vec<DevLogEntry>,
}

impl DevLogStore {
    pub fn create(session_id: impl Into<String>, agent_name: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            session_id: session_id.into(),
            agent_name: agent_name.into(),
            created_at: now.clone(),
            updated_at: now,
            entries: Vec::new(),
        }
    }

    pub fn append(&mut self, entry: DevLogEntry) {
        self.entries.push(entry);
        self.updated_at = Utc::now().to_rfc3339();
    }

    pub fn filter_entries(
        &self,
        category: Option<&str>,
        tags: Option<Vec<String>>,
        limit: Option<usize>,
    ) -> Vec<DevLogEntry> {
        let mut filtered: Vec<&DevLogEntry> = self.entries.iter().collect();

        if let Some(cat) = category {
            filtered.retain(|e| e.category == cat);
        }

        if let Some(ref tags) = tags {
            filtered.retain(|e| {
                e.metadata
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter().any(|t| {
                            tags.iter().any(|tag| t.as_str().map_or(false, |s| s == tag.as_str()))
                        })
                    })
                    .unwrap_or(false)
            });
        }

        if let Some(lim) = limit {
            if lim > 0 && lim < filtered.len() {
                filtered = filtered[(filtered.len() - lim)..].to_vec();
            }
        }

        filtered.into_iter().cloned().collect()
    }

    pub fn get_stats(&self) -> JsonValue {
        let mut by_category = serde_json::Map::new();
        for entry in &self.entries {
            let counter = by_category
                .entry(entry.category.clone())
                .or_insert_with(|| JsonValue::Number(serde_json::Number::from(0)));
            if let JsonValue::Number(ref mut n) = counter {
                *n = serde_json::Number::from(n.as_u64().unwrap_or(0) + 1);
            }
        }
        let mut map = serde_json::Map::new();
        map.insert(
            "total_entries".to_string(),
            JsonValue::Number(serde_json::Number::from(self.entries.len())),
        );
        map.insert("by_category".to_string(), JsonValue::Object(by_category));
        JsonValue::Object(map)
    }

    pub fn generate_summary(&self, limit: usize) -> String {
        if self.entries.is_empty() {
            return "📝 暂无开发日志".to_string();
        }
        let stats = self.get_stats();
        let total = self.entries.len();
        let recent = if self.entries.len() > limit {
            &self.entries[(self.entries.len() - limit)..]
        } else {
            &self.entries[..]
        };
        let mut summary_parts = vec![format!("📝 共 {} 条日志", total)];
        if let Some(by_cat) = stats.get("by_category").and_then(|v| v.as_object()) {
            let cat_summary: Vec<String> = by_cat
                .iter()
                .map(|(cat, count)| format!("{}({})", cat, count.as_u64().unwrap_or(0)))
                .collect();
            summary_parts.push(format!("分类: {}", cat_summary.join(", ")));
        }
        let recent_entries: Vec<&DevLogEntry> = recent.iter().rev().take(3).collect();
        if !recent_entries.is_empty() {
            let recent_summary: Vec<String> = recent_entries
                .iter()
                .map(|e| {
                    let truncated = safe_truncate(&e.content, 30);
                    if e.content.chars().count() > 30 {
                        format!("[{}] {}...", e.category, truncated)
                    } else {
                        format!("[{}] {}", e.category, truncated)
                    }
                })
                .collect();
            summary_parts.push(format!("最近: {}", recent_summary.join("; ")));
        }
        summary_parts.join(". ")
    }

    pub fn to_dict(&self) -> JsonValue {
        serde_json::to_value(self).unwrap_or_default()
    }

    pub fn from_dict(data: &JsonValue) -> Result<Self, serde_json::Error> {
        serde_json::from_value(data.clone())
    }
}

// ---------- DevLogTool (实现 Tool trait) ----------
pub struct DevLogTool {
    name: String,
    description: String,
    session_id: String,
    agent_name: String,
    persistence_dir: PathBuf,
    pub store: Arc<Mutex<DevLogStore>>,
}

impl DevLogTool {
    pub fn new(
        session_id: impl Into<String>,
        agent_name: impl Into<String>,
        project_root: impl Into<PathBuf>,
        persistence_dir: impl Into<PathBuf>,
    ) -> Self {
        let session_id = session_id.into();
        let agent_name = agent_name.into();
        let persistence_dir = persistence_dir.into();

        fs::create_dir_all(&persistence_dir).ok();

        let mut store = DevLogStore::create(&session_id, &agent_name);

        // 尝试加载已有日志
        let filepath = persistence_dir.join(format!("devlog-{}.json", session_id));
        if filepath.exists() {
            if let Ok(content) = fs::read_to_string(&filepath) {
                if let Ok(data) = serde_json::from_str::<JsonValue>(&content) {
                    if let Ok(loaded) = DevLogStore::from_dict(&data) {
                        store = loaded;
                    }
                }
            }
        }

        let description = format!(
            "记录开发过程中的关键决策和问题。\n\n支持的类别：\n{}\n\n操作：\n- append: 追加日志（需要 category, content, metadata）\n- read: 读取日志（可选 category, tags, limit）\n- summary: 生成摘要\n- clear: 清空日志\n\n示例：\n{{\n  \"action\": \"append\",\n  \"category\": \"decision\",\n  \"content\": \"选择使用 Redis 作为缓存层\",\n  \"metadata\": {{\"tags\": [\"architecture\", \"cache\"]}}\n}}",
            categories_str()
        );

        Self {
            name: "DevLog".to_string(),
            description,
            session_id,
            agent_name,
            persistence_dir,
            store: Arc::new(Mutex::new(store)),
        }
    }

    fn persist(&self) -> std::io::Result<()> {
        let filename = format!("devlog-{}.json", self.session_id);
        let filepath = self.persistence_dir.join(filename);
        let temp_path = filepath.with_extension("tmp");

        let store = self.store.lock().unwrap();
        let json = store.to_dict();
        fs::write(&temp_path, serde_json::to_string_pretty(&json).unwrap())?;
        fs::rename(&temp_path, &filepath)?;
        Ok(())
    }

    fn handle_append(&self, parameters: &JsonValue) -> Result<ToolResponse, HelloAgentError> {
        let category = parameters
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let content = parameters
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if category.is_empty() {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "追加日志时必须指定 category",
            ));
        }
        if !CATEGORIES.contains_key(category) {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                &format!(
                    "无效的类别：{}，支持的类别：{}",
                    category,
                    category_keys().join(", ")
                ),
            ));
        }
        if content.is_empty() {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "追加日志时必须指定 content",
            ));
        }

        let metadata: HashMap<String, JsonValue> = parameters
            .get("metadata")
            .cloned()
            .unwrap_or(JsonValue::Object(Default::default()))
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let entry = DevLogEntry::create(category, content, Some(metadata));
        let stats = {
            let mut store = self.store.lock().unwrap();
            store.append(entry.clone());
            store.get_stats()
        };
        self.persist().ok();

        let preview = safe_truncate(&entry.content, 50);
        Ok(ToolResponse::success_with(
            format!("✅ 日志已记录 [{}]: {}", entry.category, preview),
            Some(serde_json::json!({
                "log_id": entry.id,
                "timestamp": entry.timestamp,
                "category": entry.category,
            })),
            Some(stats),
            None,
        ))
    }

    fn handle_read(&self, parameters: &JsonValue) -> Result<ToolResponse, HelloAgentError> {
        let filter = parameters
            .get("filter")
            .unwrap_or(&JsonValue::Null);
        let category = filter.get("category").and_then(|v| v.as_str());
        let tags: Option<Vec<String>> = filter
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.as_str().map(|s| s.to_string()))
                    .collect()
            });
        let limit = filter
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        let store = self.store.lock().unwrap();
        let entries = store.filter_entries(category, tags, limit);

        if entries.is_empty() {
            return Ok(ToolResponse::success_with(
                "📝 未找到匹配的日志",
                Some(serde_json::json!({"entries": []})),
                Some(serde_json::json!({"matched": 0})),
                None,
            ));
        }

        let mut lines = vec![format!("📝 找到 {} 条日志：\n", entries.len())];
        for entry in &entries {
            lines.push(format!("[{}] {}", entry.category, entry.timestamp));
            lines.push(format!("  {}", entry.content));
            if !entry.metadata.is_empty() {
                lines.push(format!(
                    "  元数据: {}",
                    serde_json::to_string(&entry.metadata).unwrap_or_default()
                ));
            }
            lines.push(String::new());
        }

        Ok(ToolResponse::success_with(
            lines.join("\n"),
            Some(serde_json::json!({
                "entries": entries.iter().map(|e| e.to_dict()).collect::<Vec<_>>()
            })),
            Some(serde_json::json!({"matched": entries.len()})),
            None,
        ))
    }

    fn handle_summary(&self) -> Result<ToolResponse, HelloAgentError> {
        let store = self.store.lock().unwrap();
        let summary = store.generate_summary(10);
        let stats = store.get_stats();
        Ok(ToolResponse::success_with(summary, Some(stats), None, None))
    }

    fn handle_clear(&self) -> Result<ToolResponse, HelloAgentError> {
        let old_count;
        {
            let mut store = self.store.lock().unwrap();
            old_count = store.entries.len();
            store.entries.clear();
            store.updated_at = Utc::now().to_rfc3339();
        }
        self.persist().ok();

        Ok(ToolResponse::success_with(
            format!("✅ 已清空 {} 条日志", old_count),
            Some(serde_json::json!({"cleared_count": old_count})),
            None,
            None,
        ))
    }
}

#[async_trait]
impl Tool for DevLogTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "操作类型：append/read/summary/clear",
                    "enum": ["append", "read", "summary", "clear"]
                },
                "category": {
                    "type": "string",
                    "description": "日志类别（append 时必填）"
                },
                "content": {
                    "type": "string",
                    "description": "日志内容（append 时必填）"
                },
                "metadata": {
                    "type": "object",
                    "description": "元数据（可选）"
                },
                "filter": {
                    "type": "object",
                    "description": "过滤条件（read 时可选）"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: JsonValue) -> Result<ToolResponse, HelloAgentError> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match action {
            "append" => self.handle_append(&args),
            "read" => self.handle_read(&args),
            "summary" => self.handle_summary(),
            "clear" => self.handle_clear(),
            _ => Ok(ToolResponse::error_with(
                ToolErrorCode::InvalidParam.as_str(),
                &format!("未知操作：{}", action),
                None,
                None,
            )),
        }
    }
}