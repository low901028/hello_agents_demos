use crate::hello_agent::core::agent::Agent;
use crate::hello_agent::core::config::Config;
use crate::hello_agent::core::exceptions::HelloAgentsError;
use crate::hello_agent::core::llm::HelloAgentsLLM;
use crate::hello_agent::tools::registry::ToolRegistry;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 创建 Agent 实例
pub fn create_agent(
    agent_type: &str,
    name: &str,
    llm: HelloAgentsLLM,
    tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    config: Option<Config>,
    system_prompt: Option<&str>,
) -> Result<Box<dyn Agent>, HelloAgentsError> {
    match agent_type.to_lowercase().as_str() {
        "simple" => {
            let agent = crate::hello_agent::agents::simple_agent::SimpleAgent::new(
                name,
                llm,
                system_prompt,
                config.unwrap_or_default(),
                tool_registry,
            );
            Ok(Box::new(agent))
        }
        "react" => {
            let agent = crate::hello_agent::agents::react::ReActAgent::new(
                name,
                llm,
                tool_registry,
                system_prompt,
                config,
            );
            Ok(Box::new(agent))
        }
        "reflection" => {
            let agent = crate::hello_agent::agents::reflection::ReflectionAgent::new(
                name,
                llm,
                system_prompt,
                config.unwrap_or_default(),
                tool_registry,
            );
            Ok(Box::new(agent))
        }
        "plan" | "plansolve" => {
            let agent = crate::hello_agent::agents::plan_solve::PlanSolveAgent::new(
                name,
                llm,
                system_prompt,
                config.unwrap_or_default(),
                tool_registry,
            );
            Ok(Box::new(agent))
        }
        _ => Err(HelloAgentsError::Agent(format!(
            "不支持的 agent_type: {}. 支持: simple, react, reflection, plan",
            agent_type
        ))),
    }
}

/// 默认子代理工厂
pub fn default_subagent_factory(
    agent_type: &str,
    llm: HelloAgentsLLM,
    tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    config: Option<Config>,
) -> Result<Box<dyn Agent>, HelloAgentsError> {
    let name = format!("subagent-{}", agent_type);
    create_agent(agent_type, &name, llm, tool_registry, config, None)
}