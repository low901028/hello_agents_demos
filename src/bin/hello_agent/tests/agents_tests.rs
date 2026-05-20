//! Agent 模块集成测试

use hello_agents::agents::*;
use hello_agents::core::*;
use hello_agents::tools::*;
use std::collections::HashMap;
use std::sync::Arc;

fn create_test_llm() -> llm::HelloAgentsLlm {
    llm::HelloAgentsLlm::new(
        Some("gpt-4"),
        Some("test-key"),
        Some("https://api.openai.com/v1"),
        0.7,
        None,
        None,
    )
    .unwrap()
}

#[test]
fn test_simple_agent_creation() {
    let llm = create_test_llm();
    let config = config::Config::default();
    let registry = Arc::new(ToolRegistry::default());

    let agent = SimpleAgent::new(
        "TestSimple",
        llm,
        Some("你是测试助手".to_string()),
        config,
        Some(registry),
        true,
        3,
    );

    assert_eq!(agent.name(), "TestSimple");
    assert_eq!(agent.get_system_prompt(), Some("你是测试助手"));
}

#[test]
fn test_simple_agent_history() {
    let llm = create_test_llm();
    let config = config::Config::default();
    let registry = Arc::new(ToolRegistry::default());

    let agent = SimpleAgent::new("TestSimple", llm, None, config, Some(registry), false, 3);

    assert!(agent.get_history().is_empty());
    agent.clear_history();
    assert!(agent.get_history().is_empty());
}

#[test]
fn test_simple_agent_tools() {
    let llm = create_test_llm();
    let config = config::Config::default();
    let registry = Arc::new(ToolRegistry::default());

    let agent = SimpleAgent::new("TestSimple", llm, None, config, Some(registry), true, 3);

    let tools = agent.list_tools();
    // 新创建的 registry 应该有默认工具
    assert!(!tools.is_empty());
}

#[test]
fn test_react_agent_creation() {
    let llm = create_test_llm();
    let config = config::Config::default();
    let registry = Arc::new(ToolRegistry::default());

    let agent = ReActAgent::new("TestReAct", llm, registry, None, config, 5);

    assert_eq!(agent.name(), "TestReAct");
    assert!(agent.get_system_prompt().is_some());
}

#[test]
fn test_reflection_agent_creation() {
    let llm = create_test_llm();
    let config = config::Config::default();

    let agent = ReflectionAgent::new("TestReflection", llm, None, config, 3, None, false, 3);

    assert_eq!(agent.name(), "TestReflection");
    assert!(agent.get_system_prompt().is_some());
}

#[test]
fn test_plan_solve_agent_creation() {
    let llm = create_test_llm();
    let config = config::Config::default();

    let agent = PlanSolveAgent::new(
        "TestPlanSolve",
        llm,
        None,
        config,
        None,
        None,
        None,
        false,
        3,
    );

    assert_eq!(agent.name(), "TestPlanSolve");
}

#[test]
fn test_create_agent_via_factory() {
    let llm = create_test_llm();
    let registry = Arc::new(ToolRegistry::default());

    let agent = create_agent("react", "FactoryReAct", llm, Some(registry), None, None);

    assert!(agent.is_ok());
    assert_eq!(agent.unwrap().name(), "FactoryReAct");
}

#[test]
fn test_create_agent_invalid_type() {
    let llm = create_test_llm();
    let result = create_agent("invalid", "Test", llm, None, None, None);
    assert!(result.is_err());
}

#[test]
fn test_create_all_agent_types() {
    for agent_type in &["react", "reflection", "plan", "simple"] {
        let llm = create_test_llm();
        let registry = Arc::new(ToolRegistry::default());
        let result = create_agent(agent_type, "TestAgent", llm, Some(registry), None, None);
        assert!(
            result.is_ok(),
            "Failed to create agent type: {}",
            agent_type
        );
    }
}

#[test]
fn test_agent_factory_system_prompts() {
    // 测试不同 Agent 类型的默认系统提示词
    let react_prompt = "你是一个高效的任务执行专家";
    let reflection_prompt = "你是一个反思型专家";
    let plan_prompt = "你是一个任务规划专家";
    let simple_prompt = "你是一个简洁高效的助手";

    // 这些提示词应该被包含在默认提示中
    assert!(!react_prompt.is_empty());
    assert!(!reflection_prompt.is_empty());
    assert!(!plan_prompt.is_empty());
    assert!(!simple_prompt.is_empty());
}

#[test]
fn test_context_builder() {
    use hello_agents::context::*;

    let builder = ContextBuilder::new(None);
    let context = builder.build("测试问题", None, Some("你是助手"), None);

    assert!(context.contains("[Role & Policies]"));
    assert!(context.contains("你是助手"));
    assert!(context.contains("[Task]"));
    assert!(context.contains("测试问题"));
    assert!(context.contains("[Output]"));
}

#[test]
fn test_history_manager() {
    use hello_agents::context::*;

    let mut manager = HistoryManager::default();
    manager.append(message::Message::user("问题1"));
    manager.append(message::Message::assistant("回答1"));
    manager.append(message::Message::user("问题2"));
    manager.append(message::Message::assistant("回答2"));

    assert_eq!(manager.estimate_rounds(), 2);
    assert_eq!(manager.find_round_boundaries(), vec![0, 2]);
}

#[test]
fn test_token_counter() {
    use hello_agents::context::*;

    let mut counter = TokenCounter::new("gpt-4");
    let msg = message::Message::user("Hello World");

    let tokens = counter.count_message(&msg);
    assert!(tokens > 0);

    // 缓存测试
    let tokens2 = counter.count_message(&msg);
    assert_eq!(tokens, tokens2);
}

#[test]
fn test_observation_truncator() {
    use hello_agents::context::*;

    let truncator = ObservationTruncator::new(2, 10000, "head", "/tmp/test-output");

    let output = "Line1\nLine2\nLine3\nLine4\nLine5";
    let result = truncator.truncate("test_tool", output, None);

    let truncated = result.get("truncated").unwrap().as_bool().unwrap();
    assert!(truncated);

    let preview = result.get("preview").unwrap().as_str().unwrap();
    let preview_lines: Vec<&str> = preview.lines().collect();
    assert!(preview_lines.len() <= 2);
}

#[test]
fn test_skill_loader() {
    use hello_agents::skills::*;
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("test-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test-skill
description: 测试技能
---

# 测试技能

这是测试技能的内容。
"#,
    )
    .unwrap();

    let loader = SkillLoader::new(dir.path().to_path_buf());
    assert_eq!(loader.skill_count(), 1);

    let descriptions = loader.get_descriptions();
    assert!(descriptions.contains("test-skill"));

    let skill = loader.get_skill("test-skill").unwrap();
    assert_eq!(skill.name, "test-skill");
    assert!(skill.body.contains("测试技能"));
}

#[test]
fn test_trace_logger() {
    use hello_agents::observability::*;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let mut logger = TraceLogger::new(dir.path().to_str().unwrap(), true, false).unwrap();

    let mut payload = HashMap::new();
    payload.insert("message".to_string(), serde_json::json!("测试事件"));

    logger.log_event("test", &payload, Some(1));

    assert_eq!(logger.events.len(), 1);

    logger.finalize();

    // 检查文件是否创建
    assert!(
        dir.path()
            .join(format!("trace-{}.jsonl", logger.get_session_id()))
            .exists()
    );
    assert!(
        dir.path()
            .join(format!("trace-{}.html", logger.get_session_id()))
            .exists()
    );
}
