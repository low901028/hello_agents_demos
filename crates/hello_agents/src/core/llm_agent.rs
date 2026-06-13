//! llm_agent.rs
//! Agent 基类 - 包含 Agent trait、AgentBase 结构体及相关子代理类型

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use chrono::Utc;
use serde_json::Value;
use crate::context::history::HistoryManager;
use crate::context::truncator::{ObservationTruncator, TruncateDirection};
use crate::context::token_counter::TokenCounter;
use crate::core::config::Config;
use crate::core::hello_agents_llm::HelloAgentsLLM;
use crate::core::session_store::SessionStore;
use crate::observability::trace_logger::TraceLogger;
use crate::skills::skill_loader::SkillLoader;
use crate::tools::tool_registry::ToolRegistry;
/// Agent 基础结构体，组合所有上下文组件。
/// 具体 Agent 可通过持有此结构体并实现 Agent trait 来获得全部基础能力。
pub struct AgentBase {
    pub name: String,
    pub llm: HelloAgentsLLM,
    pub system_prompt: Option<String>,
    pub config: Config,
    pub tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    pub history_manager: HistoryManager,
    pub truncator: ObservationTruncator,
    pub token_counter: TokenCounter,
    pub history_token_count: usize,
    pub trace_logger: Option<TraceLogger>,
    pub skill_loader: Option<SkillLoader>,
    pub session_store: Option<SessionStore>,
    pub session_metadata: HashMap<String, Value>,
    pub start_time: chrono::DateTime<Utc>,
    pub max_steps: Option<usize>,
    pub working_dir: PathBuf,
}

impl AgentBase {
    pub fn new(
        name: impl Into<String>,
        llm: HelloAgentsLLM,
        system_prompt: Option<String>,
        config: Option<Config>,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        working_dir: Option<PathBuf>,
    ) -> Self {
        let config = config.unwrap_or_default();
        let history_manager = HistoryManager::new(config.min_retain_rounds, config.compression_threshold);
        let truncator = ObservationTruncator::new(
            config.tool_output_max_lines,
            config.tool_output_max_bytes,
            TruncateDirection::from_str(&config.tool_output_truncate_direction),
            &config.tool_output_dir,
        );
        let token_counter = TokenCounter::new(&llm.model);
        let trace_logger = if config.trace_enabled {
            Some(TraceLogger::new(
                &config.trace_dir,
                config.trace_sanitize,
                Some(config.trace_html_include_raw_response),
            ))
        } else {
            None
        };
        let skill_loader = if config.skills_enabled {
            Some(SkillLoader::new(&config.skills_dir))
        } else {
            None
        };
        let session_store = if config.session_enabled {
            SessionStore::new(&config.session_dir).ok()
        } else {
            None
        };

        let mut session_metadata = HashMap::new();
        session_metadata.insert("created_at".into(), Value::String(Utc::now().to_rfc3339()));
        session_metadata.insert("total_tokens".into(), Value::Number(0.into()));
        session_metadata.insert("total_steps".into(), Value::Number(0.into()));
        session_metadata.insert("duration_seconds".into(), Value::Number(0.into()));

        let mut base = Self {
            name: name.into(),
            llm,
            system_prompt,
            config,
            tool_registry: tool_registry.clone(),
            history_manager,
            truncator,
            token_counter,
            history_token_count: 0,
            trace_logger,
            skill_loader,
            session_store,
            session_metadata,
            start_time: Utc::now(),
            max_steps: None,
            working_dir: working_dir.unwrap_or_else(|| PathBuf::from(".")),
        };

        // 记录会话开始
        if let Some(ref mut logger) = base.trace_logger {
            let mut data = HashMap::<String, Value>::new();
            data.insert("agent_name".into(), Value::String(base.name.clone()));
            data.insert("agent_type".into(), Value::String("Agent".into()));
            data.insert("config".into(), serde_json::to_value(base.config.to_dict()).unwrap_or_default());
            logger.log_event("session_start", serde_json::to_value(data).unwrap_or_default(), None);
        }

        base
    }
}