// ==================== 主程序 ====================
mod simple_agent;

use std::env;
use std::sync::Arc;
use anyhow::Context;
use dotenvy::dotenv;
use crate::simple_agent::simple_agent_autogen::{create_code_reviewer, create_engineer, create_product_manager, create_user_proxy, RoundRobinGroupChat};
use crate::simple_agent::simple_agent_client::HelloAgentsLLM;
/// =========================================================== ////
/// 一个简单模拟autogen的实现(基本功能) 具体代码实现见 {{#include src/bin/simple_agent/simple_agent_autogen.rs}}
///  - PM(产品经理)  将输入的用户需求转为清晰、可执行的开发计划
///  - Engineer(工程师) 根据开发计划，编写具体的代码
///  - CodeReviewer(代码审查员) 审查工程师提交的代码，确保质量、可读性及健壮性
///  - UserProxy(用户代理) 代表用户，发起初始任务，负责执行和验证最终交付的代码
///
/// 如下代码是基于deepseek来完成的
///
/// =========================================================== ////
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    // 1、创建LLM client
    let model = "deepseek-v4-pro";
    let api_key = env::var("DEEPSEEK_API_KEY").context("请设置 DEEPSEEK_API_KEY 环境变量")?;
    let base_url =
        env::var("LLM_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com".into());
    let timeout = env::var("LLM_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);

    let llm_client = Arc::new(HelloAgentsLLM::new(
        Some(&model),
        Some(&api_key),
        Some(&base_url),
        Some(timeout),
    )?);

    // 2、创建团队智能体（注意 UserProxy 不再需要 LLM 客户端）
    let agents = vec![
        create_product_manager(llm_client.clone()),  // PM 产品经理
        create_engineer(llm_client.clone()),         // Engineer 工程师
        create_code_reviewer(llm_client.clone()),    // Code Reviewer 代码审查员
        create_user_proxy(), // 用户代理独立创建        // User Proxy 用户
    ];
    // 定义团队协作流程
    let mut chat = RoundRobinGroupChat::new(agents, "TERMINATE".into(), 20);

    // 3、用户起始需求
    let task = r#"我们需要开发一个比特币价格显示应用，具体要求如下：

核心功能：
- 实时显示比特币当前价格（USD）
- 显示24小时价格变化趋势（涨跌幅和涨跌额）
- 提供价格刷新功能

技术要求：
- 使用 Streamlit 框架创建 Web 应用
- 界面简洁美观，用户友好
- 添加适当的错误处理和加载状态

请团队协作完成这个任务，从需求分析到最终实现。"#;

    println!("🚀 启动 AutoGen 软件开发团队协作...");
    println!("{}","=".repeat(60));
    // 任务协作运行
    if let Err(e) = chat.run(task).await {
        eprintln!("协作运行错误: {}", e);
    }

    println!("✅ 团队协作结束");
    Ok(())
}
