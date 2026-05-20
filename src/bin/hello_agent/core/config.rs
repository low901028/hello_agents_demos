use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default)]
    pub max_tokens: Option<usize>,
    #[serde(default)]
    pub debug: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_max_history_length")]
    pub max_history_length: usize,
    #[serde(default = "default_context_window")]
    pub context_window: usize,
    #[serde(default = "default_compression_threshold")]
    pub compression_threshold: f64,
    #[serde(default = "default_min_retain_rounds")]
    pub min_retain_rounds: usize,
    #[serde(default)]
    pub enable_smart_compression: bool,
    #[serde(default = "default_summary_provider")]
    pub summary_llm_provider: String,
    #[serde(default = "default_summary_model")]
    pub summary_llm_model: String,
    #[serde(default = "default_summary_max_tokens")]
    pub summary_max_tokens: usize,
    #[serde(default = "default_summary_temperature")]
    pub summary_temperature: f64,
    #[serde(default = "default_tool_output_max_lines")]
    pub tool_output_max_lines: usize,
    #[serde(default = "default_tool_output_max_bytes")]
    pub tool_output_max_bytes: usize,
    #[serde(default = "default_tool_output_dir")]
    pub tool_output_dir: String,
    #[serde(default = "default_truncate_direction")]
    pub tool_output_truncate_direction: String,
    #[serde(default = "default_true")]
    pub trace_enabled: bool,
    #[serde(default = "default_trace_dir")]
    pub trace_dir: String,
    #[serde(default = "default_true")]
    pub trace_sanitize: bool,
    #[serde(default)]
    pub trace_html_include_raw_response: bool,
    #[serde(default = "default_true")]
    pub skills_enabled: bool,
    #[serde(default = "default_skills_dir")]
    pub skills_dir: String,
    #[serde(default = "default_true")]
    pub skills_auto_register: bool,
    #[serde(default = "default_true")]
    pub circuit_enabled: bool,
    #[serde(default = "default_failure_threshold")]
    pub circuit_failure_threshold: usize,
    #[serde(default = "default_recovery_timeout")]
    pub circuit_recovery_timeout: usize,
    #[serde(default = "default_true")]
    pub session_enabled: bool,
    #[serde(default = "default_session_dir")]
    pub session_dir: String,
    #[serde(default)]
    pub auto_save_enabled: bool,
    #[serde(default = "default_auto_save_interval")]
    pub auto_save_interval: usize,
    #[serde(default = "default_true")]
    pub subagent_enabled: bool,
    #[serde(default = "default_subagent_max_steps")]
    pub subagent_max_steps: usize,
    #[serde(default)]
    pub subagent_use_light_llm: bool,
    #[serde(default = "default_light_provider")]
    pub subagent_light_llm_provider: String,
    #[serde(default = "default_light_model")]
    pub subagent_light_llm_model: String,
    #[serde(default = "default_true")]
    pub todowrite_enabled: bool,
    #[serde(default = "default_todo_dir")]
    pub todowrite_persistence_dir: String,
    #[serde(default = "default_true")]
    pub devlog_enabled: bool,
    #[serde(default = "default_devlog_dir")]
    pub devlog_persistence_dir: String,
    #[serde(default = "default_true")]
    pub async_enabled: bool,
    #[serde(default = "default_max_concurrent_tools")]
    pub max_concurrent_tools: usize,
    #[serde(default = "default_hook_timeout")]
    pub hook_timeout_seconds: f64,
    #[serde(default = "default_llm_async_timeout")]
    pub llm_async_timeout: usize,
    #[serde(default = "default_tool_async_timeout")]
    pub tool_async_timeout: usize,
    #[serde(default = "default_true")]
    pub stream_enabled: bool,
    #[serde(default = "default_stream_buffer_size")]
    pub stream_buffer_size: usize,
    #[serde(default = "default_true")]
    pub stream_include_thinking: bool,
    #[serde(default = "default_true")]
    pub stream_include_tool_calls: bool,
}

fn default_model() -> String {
    "gpt-3.5-turbo".into()
}
fn default_provider() -> String {
    "openai".into()
}
fn default_temperature() -> f64 {
    0.7
}
fn default_log_level() -> String {
    "INFO".into()
}
fn default_max_history_length() -> usize {
    100
}
fn default_context_window() -> usize {
    128000
}
fn default_compression_threshold() -> f64 {
    0.8
}
fn default_min_retain_rounds() -> usize {
    10
}
fn default_summary_provider() -> String {
    "deepseek".into()
}
fn default_summary_model() -> String {
    "deepseek-chat".into()
}
fn default_summary_max_tokens() -> usize {
    800
}
fn default_summary_temperature() -> f64 {
    0.3
}
fn default_tool_output_max_lines() -> usize {
    2000
}
fn default_tool_output_max_bytes() -> usize {
    51200
}
fn default_tool_output_dir() -> String {
    "tool-output".into()
}
fn default_truncate_direction() -> String {
    "head".into()
}
fn default_true() -> bool {
    true
}
fn default_trace_dir() -> String {
    "memory/traces".into()
}
fn default_skills_dir() -> String {
    "skills".into()
}
fn default_failure_threshold() -> usize {
    3
}
fn default_recovery_timeout() -> usize {
    300
}
fn default_session_dir() -> String {
    "memory/sessions".into()
}
fn default_auto_save_interval() -> usize {
    10
}
fn default_subagent_max_steps() -> usize {
    15
}
fn default_light_provider() -> String {
    "deepseek".into()
}
fn default_light_model() -> String {
    "deepseek-chat".into()
}
fn default_todo_dir() -> String {
    "memory/todos".into()
}
fn default_devlog_dir() -> String {
    "memory/devlogs".into()
}
fn default_max_concurrent_tools() -> usize {
    3
}
fn default_hook_timeout() -> f64 {
    5.0
}
fn default_llm_async_timeout() -> usize {
    120
}
fn default_tool_async_timeout() -> usize {
    30
}
fn default_stream_buffer_size() -> usize {
    100
}

impl Default for Config {
    fn default() -> Self {
        Config {
            default_model: default_model(),
            default_provider: default_provider(),
            temperature: default_temperature(),
            max_tokens: None,
            debug: false,
            log_level: default_log_level(),
            max_history_length: default_max_history_length(),
            context_window: default_context_window(),
            compression_threshold: default_compression_threshold(),
            min_retain_rounds: default_min_retain_rounds(),
            enable_smart_compression: false,
            summary_llm_provider: default_summary_provider(),
            summary_llm_model: default_summary_model(),
            summary_max_tokens: default_summary_max_tokens(),
            summary_temperature: default_summary_temperature(),
            tool_output_max_lines: default_tool_output_max_lines(),
            tool_output_max_bytes: default_tool_output_max_bytes(),
            tool_output_dir: default_tool_output_dir(),
            tool_output_truncate_direction: default_truncate_direction(),
            trace_enabled: true,
            trace_dir: default_trace_dir(),
            trace_sanitize: true,
            trace_html_include_raw_response: false,
            skills_enabled: true,
            skills_dir: default_skills_dir(),
            skills_auto_register: true,
            circuit_enabled: true,
            circuit_failure_threshold: default_failure_threshold(),
            circuit_recovery_timeout: default_recovery_timeout(),
            session_enabled: true,
            session_dir: default_session_dir(),
            auto_save_enabled: false,
            auto_save_interval: default_auto_save_interval(),
            subagent_enabled: true,
            subagent_max_steps: default_subagent_max_steps(),
            subagent_use_light_llm: false,
            subagent_light_llm_provider: default_light_provider(),
            subagent_light_llm_model: default_light_model(),
            todowrite_enabled: true,
            todowrite_persistence_dir: default_todo_dir(),
            devlog_enabled: true,
            devlog_persistence_dir: default_devlog_dir(),
            async_enabled: true,
            max_concurrent_tools: default_max_concurrent_tools(),
            hook_timeout_seconds: default_hook_timeout(),
            llm_async_timeout: default_llm_async_timeout(),
            tool_async_timeout: default_tool_async_timeout(),
            stream_enabled: true,
            stream_buffer_size: default_stream_buffer_size(),
            stream_include_thinking: true,
            stream_include_tool_calls: true,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let mut config = Config::default();
        if let Ok(val) = env::var("DEBUG") {
            config.debug = val == "true";
        }
        if let Ok(val) = env::var("TEMPERATURE") {
            if let Ok(t) = val.parse() {
                config.temperature = t;
            }
        }
        if let Ok(val) = env::var("MAX_TOKENS") {
            if let Ok(t) = val.parse() {
                config.max_tokens = Some(t);
            }
        }
        if let Ok(val) = env::var("LLM_MODEL_ID") {
            config.default_model = val;
        }
        config
    }

    pub fn to_dict(&self) -> HashMap<String, serde_json::Value> {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::Object(map)) => map.into_iter().collect(),
            _ => HashMap::new(),
        }
    }

    pub fn get_available_tokens(&self) -> usize {
        (self.context_window as f64 * (1.0 - self.compression_threshold)) as usize
    }
}
