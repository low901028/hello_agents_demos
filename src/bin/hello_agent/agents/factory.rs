use crate::hello_agent::core::agent::Agent;
use crate::hello_agent::core::config::Config;
use crate::hello_agent::core::llm::HelloAgentsLlm;
use crate::hello_agent::tools::registry::ToolRegistry;
use std::sync::Arc;

pub fn create_agent(
    agent_type: &str,
    name: &str,
    llm: HelloAgentsLlm,
    tool_registry: Option<Arc<ToolRegistry>>,
    config: Option<Config>,
    system_prompt: Option<&str>,
) -> Result<Box<dyn Agent>, String> {
    let config = config.unwrap_or_default();
    let reg = tool_registry.unwrap_or_else(|| Arc::new(ToolRegistry::default()));
    match agent_type.to_lowercase().as_str() {
        "react" => Ok(Box::new(
            crate::hello_agent::agents::react_agent::ReActAgent::new(
                name,
                llm,
                reg,
                system_prompt.map(|s| s.into()),
                config,
                5,
            ),
        )),
        "reflection" => Ok(Box::new(
            crate::hello_agent::agents::reflection_agent::ReflectionAgent::new(
                name,
                llm,
                system_prompt.map(|s| s.into()),
                config,
                3,
                Some(reg),
                true,
                3,
            ),
        )),
        "plan" => Ok(Box::new(
            crate::hello_agent::agents::plan_solve_agent::PlanSolveAgent::new(
                name,
                llm,
                system_prompt.map(|s| s.into()),
                config,
                None,
                None,
                Some(reg),
                true,
                3,
            ),
        )),
        "simple" => Ok(Box::new(
            crate::hello_agent::agents::simple_agent::SimpleAgent::new(
                name,
                llm,
                system_prompt.map(|s| s.into()),
                config,
                Some(reg),
                true,
                3,
            ),
        )),
        _ => Err(format!("不支持的agent_type: {}", agent_type)),
    }
}

pub fn default_subagent_factory(
    agent_type: &str,
    llm: HelloAgentsLlm,
    tool_registry: Option<Arc<ToolRegistry>>,
    config: Option<Config>,
) -> Result<Box<dyn Agent>, String> {
    let config = config.unwrap_or_default();
    let prompt = match agent_type {
        "react" => "你是高效的任务执行专家。",
        "reflection" => "你是反思型专家。",
        "plan" => "你是任务规划专家。",
        _ => "你是简洁高效的助手。",
    };
    create_agent(
        agent_type,
        &format!("subagent-{}", agent_type),
        llm,
        tool_registry,
        Some(config),
        Some(prompt),
    )
}
