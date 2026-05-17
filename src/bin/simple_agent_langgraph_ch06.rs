mod simple_agent;

use anyhow::{Context, Result};
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::simple_agent::simple_agent_client::HelloAgentsLLM;
use crate::simple_agent::simple_agent_langgraph::search_agent::TavilyClient;
use crate::simple_agent::simple_agent_langgraph::tavily_search::{AgentEvent, SearchAgent};

const COLOR_RESET: &str = "\x1b[0m";
const COLOR_CYAN: &str = "\x1b[36m";
const COLOR_YELLOW: &str = "\x1b[33m";
const COLOR_GREEN: &str = "\x1b[32m";
const COLOR_RED: &str = "\x1b[31m";

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // 检查必需的环境变量
    // 1、创建LLM client
    let model = env::var("DEEPSEEK_MODEL_ID").unwrap_or_else(|_| "deepseek-v4-flash".into());
    let api_key = env::var("DEEPSEEK_API_KEY").context("请设置 DEEPSEEK_API_KEY 环境变量")?;
    let base_url =
        env::var("DEEPSEEK_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com".into());
    let timeout = env::var("LLM_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);

    let tavily_api_key = env::var("TAVILY_API_KEY")
        .map_err(|_| anyhow::anyhow!("❌ 请设置环境变量 TAVILY_API_KEY"))?;

    println!("☄️ 欢迎来到智能电子书的世界！");

    let client = Arc::new(HelloAgentsLLM::new(
        Some(&model),
        Some(api_key.as_str()),
        Some(base_url.as_str()),
        Some(timeout), // 游戏需要更长超时
    )?);

    let tavily = Arc::new(TavilyClient::new(tavily_api_key));

    let agent = Arc::new(SearchAgent::new(client, tavily));

    println!("{}🔍 智能搜索助手启动！{}", COLOR_CYAN, COLOR_RESET);
    println!("我会使用 Tavily API 为您搜索最新、最准确的信息");
    println!("支持各种问题：新闻、技术、知识问答等");
    println!("(输入 'quit' 退出)\n");

    let mut session_count = 0u32;
    loop {
        // 读取用户输入
        print!("{}🤔 您想了解什么: {}", COLOR_YELLOW, COLOR_RESET);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            continue;
        }

        if matches!(input.to_lowercase().as_str(), "quit" | "q" | "退出" | "exit") {
            println!("感谢使用！再见！👋");
            break;
        }

        session_count += 1;
        println!("\n{}============================================================{}", COLOR_GREEN, COLOR_RESET);

        // 创建 channel 接收中间状态
        let (tx, mut rx) = mpsc::unbounded_channel();

        let agent = Arc::clone(&agent);
        // 异步执行搜索
        let handle = tokio::spawn(async move {
            agent.run(input, tx).await
        });

        // 接收并显示中间状态
        while let Some(event) = rx.recv().await {
            match event {
                AgentEvent::Stage(stage, message) => {
                    match stage.as_str() {
                        "understand" => println!("{}🧠 理解阶段: {}{}", COLOR_CYAN, message, COLOR_RESET),
                        "search" => println!("{}🔍 搜索阶段: {}{}", COLOR_CYAN, message, COLOR_RESET),
                        _ => println!("{}📋 {}: {}{}", COLOR_CYAN, stage, message, COLOR_RESET),
                    }
                }
                AgentEvent::Final(answer) => {
                    println!("{}💡 最终回答:{}\n{}", COLOR_GREEN, COLOR_RESET, answer);
                }
            }
        }

        // 等待任务完成
        match handle.await? {
            Ok(_) => {}
            Err(e) => {
                eprintln!("{}❌ 发生错误: {}{}", COLOR_RED, e, COLOR_RESET);
            }
        }

        println!("{}============================================================{}", COLOR_GREEN, COLOR_RESET);
        println!();
    }

    Ok(())
}