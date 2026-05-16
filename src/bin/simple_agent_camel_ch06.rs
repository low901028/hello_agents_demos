mod simple_agent;

use anyhow::{Context, Result};
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use crate::simple_agent::simple_agent_camel::role_playing::RolePlaying;
use crate::simple_agent::simple_agent_client::HelloAgentsLLM;

const CHAT_TURN_LIMIT: usize = 30;

/// 模拟彩色打印
fn print_colored(color: &str, label: &str, content: &str) {
    println!("{}{}:\n{}\n", color, label, content);
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // 1、创建LLM client
    let model = env::var("DEEPSEEK_MODEL_ID").unwrap_or_else(|_| "deepseek-v4-flash".into());
    let api_key = env::var("DEEPSEEK_API_KEY").context("请设置 DEEPSEEK_API_KEY 环境变量")?;
    let base_url =
        env::var("DEEPSEEK_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com".into());
    let timeout = env::var("LLM_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);

    println!("☄️ 欢迎来到智能电子书的世界！");

    let client = Arc::new(HelloAgentsLLM::new(
        Some(&model),
        Some(api_key.as_str()),
        Some(base_url.as_str()),
        Some(timeout), // 游戏需要更长超时
    )?);

    // 定义协作任务
    let task_prompt = r#"创作一本关于"拖延症心理学"的短篇电子书，目标读者是对心理学感兴趣的普通大众。
要求：
1. 内容科学严谨，基于实证研究
2. 语言通俗易懂，避免过多专业术语
3. 包含实用的改善建议和案例分析
4. 篇幅控制在8000-10000字
5. 结构清晰，包含引言、核心章节和总结"#;

    println!("协作任务:\n{}\n", task_prompt);

    // 初始化角色扮演会话
    let mut session = RolePlaying::new(
        "心理学家",
        "作家",
        task_prompt,
        client,
    );

    println!("具体任务描述:\n{}\n", session.task_prompt());

    // 初始化对话
    let mut input_msg = session.init_chat().await?;
    let mut n = 0;

    while n < CHAT_TURN_LIMIT {
        n += 1;

        let step_result = session.step(&input_msg).await?;

        print_colored("\x1b[34m", "作家", &step_result.user.content);
        print_colored("\x1b[32m", "心理学家", &step_result.assistant.content);

        // 检查任务完成标志
        if step_result.user.content.contains("CAMEL_TASK_DONE") {
            println!("\x1b[35m✅ 电子书创作完成！");
            break;
        }

        input_msg = step_result.assistant;
    }

    println!("总共进行了 {} 轮协作对话", n);
    Ok(())
}