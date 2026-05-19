use serde::{Deserialize, Serialize};
use std::env;

/// HelloAgents 配置类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // ========== LLM 配置 ==========
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    pub max_tokens: Option<u32>,

    // ========== 系统配置 ==========
    #[serde(default)]
    pub debug: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,

    // ========== 历史管理 ==========
    #[serde(default = "default_max_history")]
    pub max_history_length: usize,

    // ========== 上下文工程 ==========
    #[serde(default = "default_context_window")]
    pub context_window: usize,
    #[serde(default = "default_compression_threshold")]
    pub compression_threshold: f64,
    #[serde(default = "default_min_retain_rounds")]
    pub min_retain_rounds: usize,
    #[serde(default)]
    pub enable_smart_compression: bool,

    // ========== 智能摘要 ==========
    #[serde(default = "default_summary_provider")]
    pub summary_llm_provider: String,
    #[serde(default = "default_summary_model")]
    pub summary_llm_model: String,
    #[serde(default = "default_summary_max_tokens")]
    pub summary_max_tokens: u32,
    #[serde(default = "default_summary_temperature")]
    pub summary_temperature: f64,

    // ========== 工具输出截断 ==========
    #[serde(default = "default_tool_output_max_lines")]
    pub tool_output_max_lines: usize,
    #[serde(default = "default_tool_output_max_bytes")]
    pub tool_output_max_bytes: usize,
    #[serde(default = "default_tool_output_dir")]
    pub tool_output_dir: String,
    #[serde(default = "default_tool_output_truncate_direction")]
    pub tool_output_truncate_direction: String,

    // ========== 可观测性 ==========
    #[serde(default = "default_true")]
    pub trace_enabled: bool,
    #[serde(default = "default_trace_dir")]
    pub trace_dir: String,
    #[serde(default = "default_true")]
    pub trace_sanitize: bool,
    #[serde(default)]
    pub trace_html_include_raw_response: bool,

    // ========== Skills ==========
    #[serde(default = "default_true")]
    pub skills_enabled: bool,
    #[serde(default = "default_skills_dir")]
    pub skills_dir: String,
    #[serde(default = "default_true")]
    pub skills_auto_register: bool,

    // ========== 熔断器 ==========
    #[serde(default = "default_true")]
    pub circuit_enabled: bool,
    #[serde(default = "default_failure_threshold")]
    pub circuit_failure_threshold: u32,
    #[serde(default = "default_recovery_timeout")]
    pub circuit_recovery_timeout: u64,

    // ========== 会话持久化 ==========
    #[serde(default = "default_true")]
    pub session_enabled: bool,
    #[serde(default = "default_session_dir")]
    pub session_dir: String,
    #[serde(default)]
    pub auto_save_enabled: bool,
    #[serde(default = "default_auto_save_interval")]
    pub auto_save_interval: usize,

    // ========== 子代理 ==========
    #[serde(default = "default_true")]
    pub subagent_enabled: bool,
    #[serde(default = "default_subagent_max_steps")]
    pub subagent_max_steps: usize,
    #[serde(default)]
    pub subagent_use_light_llm: bool,
    #[serde(default = "default_summary_provider")]
    pub subagent_light_llm_provider: String,
    #[serde(default = "default_summary_model")]
    pub subagent_light_llm_model: String,

    // ========== TodoWrite ==========
    #[serde(default = "default_true")]
    pub todowrite_enabled: bool,
    #[serde(default = "default_todowrite_dir")]
    pub todowrite_persistence_dir: String,

    // ========== DevLog ==========
    #[serde(default = "default_true")]
    pub devlog_enabled: bool,
    #[serde(default = "default_devlog_dir")]
    pub devlog_persistence_dir: String,

    // ========== 异步生命周期 ==========
    #[serde(default = "default_true")]
    pub async_enabled: bool,
    #[serde(default = "default_max_concurrent_tools")]
    pub max_concurrent_tools: usize,
    #[serde(default = "default_hook_timeout")]
    pub hook_timeout_seconds: f64,
    #[serde(default = "default_llm_async_timeout")]
    pub llm_async_timeout: u64,
    #[serde(default = "default_tool_async_timeout")]
    pub tool_async_timeout: u64,

    // ========== 流式输出 ==========
    #[serde(default = "default_true")]
    pub stream_enabled: bool,
    #[serde(default = "default_stream_buffer_size")]
    pub stream_buffer_size: usize,
    #[serde(default = "default_true")]
    pub stream_include_thinking: bool,
    #[serde(default = "default_true")]
    pub stream_include_tool_calls: bool,
}

// ========== 默认值函数 ==========
fn default_model() -> String { "gpt-3.5-turbo".into() }
fn default_provider() -> String { "openai".into() }
fn default_temperature() -> f64 { 0.7 }
fn default_log_level() -> String { "INFO".into() }
fn default_max_history() -> usize { 100 }
fn default_context_window() -> usize { 128000 }
fn default_compression_threshold() -> f64 { 0.8 }
fn default_min_retain_rounds() -> usize { 10 }
fn default_summary_provider() -> String { "deepseek".into() }
fn default_summary_model() -> String { "deepseek-chat".into() }
fn default_summary_max_tokens() -> u32 { 800 }
fn default_summary_temperature() -> f64 { 0.3 }
fn default_tool_output_max_lines() -> usize { 2000 }
fn default_tool_output_max_bytes() -> usize { 51200 }
fn default_tool_output_dir() -> String { "tool-output".into() }
fn default_tool_output_truncate_direction() -> String { "head".into() }
fn default_trace_dir() -> String { "memory/traces".into() }
fn default_skills_dir() -> String { "skills".into() }
fn default_failure_threshold() -> u32 { 3 }
fn default_recovery_timeout() -> u64 { 300 }
fn default_session_dir() -> String { "memory/sessions".into() }
fn default_auto_save_interval() -> usize { 10 }
fn default_subagent_max_steps() -> usize { 15 }
fn default_todowrite_dir() -> String { "memory/todos".into() }
fn default_devlog_dir() -> String { "memory/devlogs".into() }
fn default_max_concurrent_tools() -> usize { 3 }
fn default_hook_timeout() -> f64 { 5.0 }
fn default_llm_async_timeout() -> u64 { 120 }
fn default_tool_async_timeout() -> u64 { 30 }
fn default_stream_buffer_size() -> usize { 100 }
fn default_true() -> bool { true }

impl Default for Config {
    fn default() -> Self {
        Self {
            default_model: default_model(),
            default_provider: default_provider(),
            temperature: default_temperature(),
            max_tokens: None,
            debug: false,
            log_level: default_log_level(),
            max_history_length: default_max_history(),
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
            tool_output_truncate_direction: default_tool_output_truncate_direction(),
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
            subagent_light_llm_provider: default_summary_provider(),
            subagent_light_llm_model: default_summary_model(),
            todowrite_enabled: true,
            todowrite_persistence_dir: default_todowrite_dir(),
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
    /// 从环境变量创建配置
    pub fn from_env() -> Self {
        let debug = env::var("DEBUG").unwrap_or_default().to_lowercase() == "true";
        let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "INFO".into());
        let temperature = env::var("TEMPERATURE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.7);
        let max_tokens = env::var("MAX_TOKENS").ok().and_then(|v| v.parse().ok());

        Self {
            debug,
            log_level,
            temperature,
            max_tokens,
            ..Default::default()
        }
    }

    /// 转换为字典
    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}