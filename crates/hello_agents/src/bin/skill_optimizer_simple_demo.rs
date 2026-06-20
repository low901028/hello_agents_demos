use std::sync::Arc;
use hello_agents::core::agent_runtime::AgentRuntime;
use hello_agents::infra::openai_adapter::OpenAIAdapter;
use hello_agents::infra::tool_registry_impl::ToolRegistryImpl;
use hello_agents::context::history_manager_impl::HistoryManagerImpl;
use hello_agents::core::traits::history::HistoryManager;
use hello_agents::core::traits::skill_optimizer::SkillOptimizer;
use hello_agents::core::traits::tool::Tool;
use hello_agents::infra::skill_optimizer_impl::LLMDrivenSkillOptimizer;
use hello_agents::infra::skill_opt_config::SkillOptConfig;
use hello_agents::core::types::config::Config;
use hello_agents::core::types::skill_opt::{Skill, Task};
use hello_agents::skills::skill_loader::SkillLoader;
use hello_agents::tools::builtin::skill::SkillTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let target_model = std::env::var("LLM_MODEL_ID").unwrap_or_else(|_| "deepseek-v4-flash".to_string());
    let api_key = std::env::var("LLM_API_KEY").expect("LLM_API_KEY not set");
    let base_url =
        std::env::var("LLM_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".to_string());

    let optimizer_model = "deepseek-v4-pro";

    let target_llm = Arc::new(OpenAIAdapter::new(&api_key, &base_url, &target_model));
    let optimizer_llm = Arc::new(OpenAIAdapter::new(&api_key, &base_url, optimizer_model));

    let tools = Arc::new(ToolRegistryImpl::new());
    let history = Arc::new(std::sync::Mutex::new(Box::new(HistoryManagerImpl::new(10, 0.8)) as Box<dyn HistoryManager>));

    let skill_loader = Arc::new(std::sync::Mutex::new(
        SkillLoader::new("skills")
    ));

    let skill_opt = Arc::new(LLMDrivenSkillOptimizer::new(
        target_llm.clone(), optimizer_llm, skill_loader.clone(), SkillOptConfig::default(),
    ));

    let _ = AgentRuntime::new(target_llm, tools, history, Config::default())
        .with_skill_optimizer(skill_opt.clone());
    // 创建 SkillTool 并注入优化器
    let skill_tool = SkillTool::new(skill_loader.clone()).with_skill_optimizer(skill_opt.clone());

    // 从 SkillLoader 加载技能，并转换为 SkillOpt 使用的 Skill 类型
    // 安全地从 SkillLoader 获取技能并转换为 SkillOpt 所需类型
    let mut guard = skill_loader.lock().unwrap();
    let loaded_skill = guard.get_skill("pdf"); // 返回 Option<Skill>，owned
    let mut skill: Skill = if let Some(s) = loaded_skill {
        Skill {
            name: s.name.clone(),
            description: s.description.clone(),
            body: s.body.clone(),
            dir: s.dir.to_string_lossy().into_owned(),
        }
    } else {
        Skill {
            name: "pdf".into(),
            description: "PDF 处理技能".into(),
            body: "初始技能内容：处理 PDF 文件。".into(),
            dir: "skills/pdf".into(),
        }
    };
    drop(guard); // 释放锁

    let train_tasks: Vec<Task> = (0..20)
        .map(|i| Task {
            id: format!("train_{}", i),
            input: format!("处理 PDF 任务 {}", i),
            expected_output: None,
            metadata: serde_json::Value::Null,
        })
        .collect();

    let val_tasks: Vec<Task> = (0..5)
        .map(|i| Task {
            id: format!("val_{}", i),
            input: format!("验证任务 {}", i),
            expected_output: None,
            metadata: serde_json::Value::Null,
        })
        .collect();

    // 调用一次技能工具（自动记录反馈）
    let _ = skill_tool.execute(serde_json::json!({"skill": "pdf"})).await?;

    println!("=========== skill evolve_step");
    let improved = skill_opt.evolve_step(&mut skill, &train_tasks, &val_tasks, 4).await?;
    println!("技能是否改进: {}", improved);
    println!("优化后技能内容:\n{}", skill.body);

    if let Some(stats) = skill_opt.get_stats("pdf") {
        println!(
            "技能统计: 调用次数={}, 成功率={:.1}%, 平均延迟={:.0}ms",
            stats.call_count,
            stats.success_rate * 100.0,
            stats.avg_latency_ms
        );
    }

    Ok(())
}