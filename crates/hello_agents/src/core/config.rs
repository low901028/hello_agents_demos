use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

/// ======================================================
///！ 项目不同配置项
/// ======================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // 默认LLM配置
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default)]
    pub max_tokens: Option<usize>,

    // 系统配置
    #[serde(default)]
    pub debug: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_max_history_length")]

    // 历史管理配置
    // 默认值:100
    pub max_history_length: usize,
    #[serde(default = "default_context_window")]

    // context配置
    // 1、上下文窗口大小(默认值)
    pub context_window: usize,
    #[serde(default = "default_compression_threshold")]
    // 2、压缩阈值(默认0.8 当达到80%时触发压缩)
    pub compression_threshold: f64,
    #[serde(default = "default_min_retain_rounds")]
    // 3、当压缩时保留的最小完整轮次(默认=10)
    pub min_retain_rounds: usize,
    // 4、是否开启智能摘要(需要额外的LLM调用，具体见下面的配置)
    #[serde(default)]
    pub enable_smart_compression: bool,

    // 智能摘要配置
    // 1、摘要专用LLM(默认使用deepseek)
    #[serde(default = "default_summary_provider")]
    pub summary_llm_provider: String,
    // 2、摘要摘要专用LLM模型(默认deepseek-flash)
    #[serde(default = "default_summary_model")]
    pub summary_llm_model: String,
    // 3、摘要最大的token数(默认800)
    #[serde(default = "default_summary_max_tokens")]
    pub summary_max_tokens: usize,
    // 4、摘要生成的温度(更确定性)
    #[serde(default = "default_summary_temperature")]
    pub summary_temperature: f64,

    // 工具配置
    // - 输出截断配置
    // 1、工具输出最大行数
    #[serde(default = "default_tool_output_max_lines")]
    pub tool_output_max_lines: usize,
    // 2、工具输出最大字节数
    #[serde(default = "default_tool_output_max_bytes")]
    pub tool_output_max_bytes: usize,
    // 3、完整输出保存目录(默认tool-output)
    #[serde(default = "default_tool_output_dir")]
    pub tool_output_dir: String,
    // 4、工具输出截断方向(可取值：head/tail/head_tail)
    #[serde(default = "default_truncate_direction")]
    pub tool_output_truncate_direction: String,
    #[serde(default = "default_true")]

    // 可观测性配置
    // 1、是否启用Trace记录(默认=true，开启)
    pub trace_enabled: bool,
    #[serde(default = "default_trace_dir")]
    // 2、Trace文件保存目录(默认：memory/traces)
    pub trace_dir: String,
    #[serde(default = "default_true")]
    // 3、敏感信息是否脱敏(默认=true，脱敏)
    pub trace_sanitize: bool,
    // 4、HTML是否包含原始响应(默认=false)
    #[serde(default)]
    pub trace_html_include_raw_response: bool,

    // Skills知识外化配置
    // 1、是否启用Skills系统
    #[serde(default = "default_true")]
    pub skills_enabled: bool,
    // 2、Skills目录路径(默认=“skills”)
    #[serde(default = "default_skills_dir")]
    pub skills_dir: String,
    // 3、是否自动注册SkillTool
    #[serde(default = "default_true")]
    pub skills_auto_register: bool,

    // 熔断器配置
    // 1、是否启动熔断器(默认=true, 开启)
    #[serde(default = "default_true")]
    pub circuit_enabled: bool,
    // 2、连续失败多少次后熔断(默认=3)
    #[serde(default = "default_failure_threshold")]
    pub circuit_failure_threshold: usize,
    // 3、熔断后恢复时间(单位：秒，默认=300)
    #[serde(default = "default_recovery_timeout")]
    pub circuit_recovery_timeout: usize,

    // Session持久化配置
    // 1、是否开启会话持久化(默认=true)
    #[serde(default = "default_true")]
    pub session_enabled: bool,
    // 2、session文件保存目录(默认=“memory/sessions”)
    #[serde(default = "default_session_dir")]
    pub session_dir: String,
    // 3、是否启用自动保存(默认false)
    #[serde(default)]
    pub auto_save_enabled: bool,
    // 4、若是启用自动保存，每隔多少条消息自动保存(默认=10)
    #[serde(default = "default_auto_save_interval")]
    pub auto_save_interval: usize,

    // 子agent机制配置
    // 1、是否开启用子代理机制
    #[serde(default = "default_true")]
    pub subagent_enabled: bool,
    // 2、子代理默认最大步数
    #[serde(default = "default_subagent_max_steps")]
    pub subagent_max_steps: usize,
    // 3、是否使用轻量模型(默认=false，关闭，为了避免破坏现有的行为)
    #[serde(default)]
    pub subagent_use_light_llm: bool,
    // 4、子agent使用的轻量模型信息
    // - 轻量模型提供商: 默认deepseek
    #[serde(default = "default_light_provider")]
    pub subagent_light_llm_provider: String,
    // - 轻量模型名称: 默认deepseek-flash
    #[serde(default = "default_light_model")]
    pub subagent_light_llm_model: String,

    // TodoWrite 进程管理
    // 1、是否启用TodoWrite工具
    #[serde(default = "default_true")]
    pub todowrite_enabled: bool,
    // 2、任务列表持久化目录
    #[serde(default = "default_todo_dir")]
    pub todowrite_persistence_dir: String,

    // Devlog开发日志配置
    // 1、是否启用Devlog工具(默认启用)
    #[serde(default = "default_true")]
    pub devlog_enabled: bool,
    // 2、开发日志持久化目录
    #[serde(default = "default_devlog_dir")]
    pub devlog_persistence_dir: String,

    // 异步生命周期配置
    // 1、是否启用异步执行； 默认=true, 启用
    #[serde(default = "default_true")]
    pub async_enabled: bool,
    // 2、最大并发工具数
    #[serde(default = "default_max_concurrent_tools")]
    pub max_concurrent_tools: usize,
    // 3、生命周期钩子的超时时间(默认5秒)
    #[serde(default = "default_hook_timeout")]
    pub hook_timeout_seconds: f64,
    // 4、LLM异步调用超时时间(120秒)
    #[serde(default = "default_llm_async_timeout")]
    pub llm_async_timeout: usize,
    // 5、工具异步调用超时时间(30秒)
    #[serde(default = "default_tool_async_timeout")]
    pub tool_async_timeout: usize,

    // 流式配置
    // 1. 是否开启streaming； 默认=true，开启
    #[serde(default = "default_true")]
    pub stream_enabled: bool,
    // 2. 流式缓冲区大小
    #[serde(default = "default_stream_buffer_size")]
    pub stream_buffer_size: usize,
    // 3、是否包含think过程(默认=true，包括)
    #[serde(default = "default_true")]
    pub stream_include_thinking: bool,
    // 4、是否包含工具调用(默认=true, 包括)
    #[serde(default = "default_true")]
    pub stream_include_tool_calls: bool,
}

fn default_model() -> String {
    "deepseek-v4-flash".into()
}
fn default_provider() -> String {
    "deepseek".into()
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
    /// 根据env文件获取对应的变量内容
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

    ///  转换为字典格式，方便日志记录
    pub fn to_dict(&self) -> Option<HashMap<String, serde_json::Value>> {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::Object(map)) => Some(map.into_iter().collect()),
            _ => None,
        }
    }

    pub fn get_available_tokens(&self) -> usize {
        (self.context_window as f64 * (1.0 - self.compression_threshold)) as usize
    }
}
