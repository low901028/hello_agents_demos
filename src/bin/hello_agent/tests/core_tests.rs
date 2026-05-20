//! 核心模块集成测试

use hello_agents::core::*;
use hello_agents::tools::*;
use std::collections::HashMap;

#[test]
fn test_message_creation_and_conversion() {
    let msg = message::Message::user("你好，世界");
    assert_eq!(msg.role, message::MessageRole::User);
    assert_eq!(msg.content, "你好，世界");

    let dict = msg.to_openai_dict();
    assert_eq!(dict.get("role").unwrap().as_str().unwrap(), "user");
    assert_eq!(dict.get("content").unwrap().as_str().unwrap(), "你好，世界");
}

#[test]
fn test_config_default_values() {
    let config = config::Config::default();
    assert_eq!(config.default_model, "gpt-3.5-turbo");
    assert_eq!(config.temperature, 0.7);
    assert_eq!(config.context_window, 128000);
    assert!(config.trace_enabled);
    assert!(config.skills_enabled);
    assert_eq!(config.max_concurrent_tools, 3);
}

#[test]
fn test_config_from_env() {
    // 设置环境变量
    std::env::set_var("DEBUG", "true");
    std::env::set_var("TEMPERATURE", "0.5");

    let config = config::Config::from_env();
    assert!(config.debug);
    assert_eq!(config.temperature, 0.5);

    // 清理
    std::env::remove_var("DEBUG");
    std::env::remove_var("TEMPERATURE");
}

#[test]
fn test_llm_response_creation() {
    let mut usage = HashMap::new();
    usage.insert("total_tokens".to_string(), 200);
    usage.insert("prompt_tokens".to_string(), 50);
    usage.insert("completion_tokens".to_string(), 150);

    let response = llm_response::LlmResponse::new("测试响应", "gpt-4", usage, 500);

    assert_eq!(response.total_tokens(), 200);
    assert_eq!(response.prompt_tokens(), 50);
    assert_eq!(response.completion_tokens(), 150);
    assert!(!response.has_reasoning());
}

#[test]
fn test_llm_response_with_reasoning() {
    let usage = HashMap::new();
    let response = llm_response::LlmResponse::with_reasoning(
        "最终答案",
        "deepseek-reasoner",
        usage,
        1000,
        "推理过程...",
    );

    assert!(response.has_reasoning());
    assert_eq!(response.reasoning_content, Some("推理过程...".to_string()));
}

#[test]
fn test_tool_response_success() {
    let mut data = HashMap::new();
    data.insert("result".to_string(), serde_json::json!(42));

    let response = response::ToolResponse::success("计算完成", data);
    assert!(response.is_success());
    assert!(!response.is_error());
    assert_eq!(response.text, "计算完成");
}

#[test]
fn test_tool_response_error() {
    let response = response::ToolResponse::error("NOT_FOUND", "工具不存在");
    assert!(response.is_error());
    let error_info = response.error_info.unwrap();
    assert_eq!(error_info.code, "NOT_FOUND");
}

#[test]
fn test_tool_response_partial() {
    let mut data = HashMap::new();
    data.insert("truncated".to_string(), serde_json::json!(true));

    let response = response::ToolResponse::partial("部分结果", data);
    assert!(response.is_partial());
}

#[test]
fn test_tool_error_codes() {
    assert_eq!(errors::ToolErrorCode::NotFound.as_str(), "NOT_FOUND");
    assert_eq!(errors::ToolErrorCode::CircuitOpen.as_str(), "CIRCUIT_OPEN");
    assert!(errors::ToolErrorCode::is_valid_code("EXECUTION_ERROR"));
    assert!(!errors::ToolErrorCode::is_valid_code("UNKNOWN"));
}

#[test]
fn test_circuit_breaker() {
    let mut cb = circuit_breaker::CircuitBreaker::new(2, 300, true);

    // 第一次失败
    cb.record_result("tool1", &response::ToolResponse::error("ERROR", "错误"));
    assert!(!cb.is_open("tool1"));

    // 第二次失败 - 触发熔断
    cb.record_result("tool1", &response::ToolResponse::error("ERROR", "错误"));
    assert!(cb.is_open("tool1"));

    // 恢复
    cb.close("tool1");
    assert!(!cb.is_open("tool1"));
}

#[test]
fn test_tool_filter_read_only() {
    let filter = tool_filter::ReadOnlyFilter::default();
    assert!(filter.is_allowed("Read"));
    assert!(filter.is_allowed("Skill"));
    assert!(!filter.is_allowed("Write"));
    assert!(!filter.is_allowed("Bash"));
}

#[test]
fn test_tool_filter_full_access() {
    let filter = tool_filter::FullAccessFilter::default();
    assert!(filter.is_allowed("Read"));
    assert!(filter.is_allowed("Write"));
    assert!(!filter.is_allowed("Bash"));
}

#[test]
fn test_tool_filter_custom_whitelist() {
    let filter =
        tool_filter::CustomFilter::whitelist(vec!["Read".to_string(), "Write".to_string()]);
    assert!(filter.is_allowed("Read"));
    assert!(!filter.is_allowed("Skill"));
}

#[test]
fn test_tool_filter_custom_blacklist() {
    let filter = tool_filter::CustomFilter::blacklist(vec!["Bash".to_string()]);
    assert!(filter.is_allowed("Read"));
    assert!(!filter.is_allowed("Bash"));
}
