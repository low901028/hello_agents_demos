use crate::hello_agent::agents::factory::{create_agent, default_subagent_factory};
use crate::hello_agent::agents::react_agent::ReActAgent;
use crate::hello_agent::core::agent::Agent;
use crate::hello_agent::core::config::Config;
use crate::hello_agent::core::llm::HelloAgentsLlm;
use crate::hello_agent::tools::builtin::file_tool::{EditTool, ReadTool, WriteTool};
use crate::hello_agent::tools::builtin::task_tool::TaskTool;
use crate::hello_agent::tools::registry::ToolRegistry;
use crate::hello_agent::tools::tool_filter::{CustomFilter, FullAccessFilter, ReadOnlyFilter, ToolFilter};
use std::sync::Arc;
use crate::hello_agent::agents::simple_agent::SimpleAgent;
use anyhow::{ Result, Context};
use dotenvy::dotenv;

#[tokio::main]
pub async fn main() -> Result<()>{
    dotenv().ok();
    
    println!("\n{}", "=".repeat(60));
    println!("HelloAgents 子代理机制示例 (Rust)");
    println!("{}", "=".repeat(60));

    let model = std::env::var("LLM_MODEL_ID").unwrap_or_else(|_| "未设置".into());
    let api_ok = std::env::var("LLM_API_KEY").is_ok();
    let base_url = std::env::var("LLM_BASE_URL").unwrap_or_else(|_| "未设置".into());
    println!("LLM_MODEL_ID: {}", model);
    println!(
        "LLM_API_KEY: {}",
        if api_ok { "已设置" } else { "❌ 未设置" }
    );
    println!("LLM_BASE_URL: {}", base_url);

    example_1();
    example_2();
    example_3();
    example_4();
    example_5();

    println!("\n{}", "=".repeat(60));
    println!("所有示例运行完成");
    println!("{}", "=".repeat(60));

    Ok(())
}

fn example_1() {
    println!("\n示例1：基本子代理使用");
    let llm = HelloAgentsLlm::new(None, None, None, 0.7, None, None).expect("LLM初始化失败");
    let registry = Arc::new(ToolRegistry::default());
    registry.register_tool(Arc::new(ReadTool::new("./", None, None)), false);
    let config = Config {
        subagent_enabled: true,
        ..Config::default()
    };
    let _agent = ReActAgent::new("main-agent", llm, registry.clone(), None, config, 5);
    println!("主Agent可用工具: {:?}", registry.list_tools());
    println!("✅ Task工具已自动注册");
}

fn example_2() {
    println!("\n示例2：手动使用子代理");
    let llm = HelloAgentsLlm::new(None, None, None, 0.7, None, None).expect("LLM初始化失败");
    let config = Config {
        subagent_enabled: false,
        skills_enabled: false,
        ..Config::default()
    };
    let registry = Arc::new(ToolRegistry::default());
    registry.register_tool(Arc::new(ReadTool::new("./", None, None)), false);
    let main_agent = ReActAgent::new(
        "main",
        llm.clone(),
        registry.clone(),
        None,
        config.clone(),
        5,
    );
    let explore_agent = ReActAgent::new("explorer", llm, registry, None, config, 5);
    let readonly = ReadOnlyFilter::default();
    let result = explore_agent.run_as_subagent("列出当前目录的文件", Some(&readonly), true, None);
    println!("子代理执行结果:");
    println!(
        "  成功: {}",
        result
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    );
    println!("  主Agent历史长度: {}", main_agent.get_history().len());
}

fn example_3() {
    println!("\n示例3：自定义子代理工厂");
    let main_llm = HelloAgentsLlm::new(None, None, None, 0.7, None, None).expect("LLM初始化失败");
    let light_llm = HelloAgentsLlm::new(
        Some("deepseek-chat"),
        None,
        Some("https://api.deepseek.com/v1"),
        0.7,
        None,
        None,
    )
    .expect("轻量LLM初始化失败");
    let registry = Arc::new(ToolRegistry::default());
    registry.register_tool(Arc::new(ReadTool::new("./", None, None)), false);
    let m_llm = main_llm.clone();
    let l_llm = light_llm.clone();
    let reg_c = registry.clone();
    let factory = Arc::new(move |t: &str| {
        let llm = match t {
            "react" | "plan" => l_llm.clone(),
            _ => m_llm.clone(),
        };
        default_subagent_factory(t, llm, Some(reg_c.clone()), Some(Config::default()))
            .unwrap_or_else(|_| {
                Box::new(SimpleAgent::new(
                    "fb",
                    l_llm.clone(),
                    None,
                    Config::default(),
                    Some(reg_c.clone()),
                    false,
                    3,
                ))
            })
    });
    registry.register_tool(
        Arc::new(TaskTool::new(factory, Some(registry.clone()), None)),
        false,
    );
    println!("✅ 自定义TaskTool已注册");
}

fn example_4() {
    println!("\n示例4：不同类型的子代理");
    let llm = HelloAgentsLlm::new(None, None, None, 0.7, None, None).expect("LLM初始化失败");
    let config = Config::default();
    let registry = Arc::new(ToolRegistry::default());
    for t in &["react", "reflection", "plan", "simple"] {
        match create_agent(
            t,
            &format!("{}-sub", t),
            llm.clone(),
            Some(registry.clone()),
            Some(config.clone()),
            None,
        ) {
            Ok(a) => println!("  - {}: {}", t, a.name()),
            Err(e) => println!("  - {}: 失败 - {}", t, e),
        }
    }
}

fn example_5() {
    println!("\n示例5：工具过滤策略");
    let registry = Arc::new(ToolRegistry::default());
    registry.register_tool(Arc::new(ReadTool::new("./", None, None)), false);
    registry.register_tool(Arc::new(WriteTool::new("./", None, None)), false);
    registry.register_tool(Arc::new(EditTool::new("./", None, None)), false);
    let all = registry.list_tools();
    let toolfilter: Box<dyn ToolFilter> = Box::new(ReadOnlyFilter::default());
    println!(
        "ReadOnlyFilter允许: {:?}",
        toolfilter.filter(&all)
    );
    println!(
        "FullAccessFilter允许: {:?}",
        FullAccessFilter::default().filter(&all)
    );
    println!(
        "CustomFilter白名单允许: {:?}",
        CustomFilter::whitelist(vec!["Read".into()]).filter(&all)
    );
    println!(
        "CustomFilter黑名单允许: {:?}",
        CustomFilter::blacklist(vec!["Write".into()]).filter(&all)
    );
}
