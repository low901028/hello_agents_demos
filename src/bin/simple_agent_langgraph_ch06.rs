mod simple_agent;

use anyhow::{Context, Result};
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use tokio::sync::mpsc;
use std::io::{self, Read, Write};
use std::borrow::Cow;
use crate::simple_agent::simple_agent_client::HelloAgentsLLM;
use crate::simple_agent::simple_agent_langgraph::lang_graph::{GraphState, Message, StreamEvent};
use crate::simple_agent::simple_agent_langgraph::search_agent::{SearchAssistant, SearchState};
use crate::simple_agent::simple_agent_langgraph::tavily_search::{TavilyClient};

const COLOR_RESET: &str = "\x1b[0m";
const COLOR_CYAN: &str = "\x1b[36m";
const COLOR_YELLOW: &str = "\x1b[33m";
const COLOR_GREEN: &str = "\x1b[32m";
const COLOR_RED: &str = "\x1b[31m";

pub fn read_input_cow(prompt: &str) -> io::Result<Cow<'static, str>> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut buffer = Vec::with_capacity(256);
    let stdin = io::stdin();
    let mut handle = stdin.lock();

    for byte in handle.bytes() {
        let b = byte?;
        if b == b'\n' {
            break;
        }
        buffer.push(b);
    }

    // 尝试高效转换
    let trimmed = match str::from_utf8(&buffer) {
        Ok(text) => {
            let trimmed = text.trim();
            if trimmed.len() == buffer.len() {
                // 没有trim操作或全是ASCII，可以安全使用Cow::Borrowed
                Cow::Owned(trimmed.to_string())
            } else {
                Cow::Owned(trimmed.to_string())
            }
        }
        Err(e) => {
            eprintln!("⚠️  警告：检测到非UTF-8字符: {}", e);
            let valid = String::from_utf8_lossy(&buffer);
            Cow::Owned(valid.trim().to_string())
        }
    };

    Ok(trimmed)
}

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

    println!("☄️ 欢迎来到智能AI的世界！");

    let client = Arc::new(HelloAgentsLLM::new(
        Some(&model),
        Some(api_key.as_str()),
        Some(base_url.as_str()),
        Some(timeout), // 游戏需要更长超时
    )?);

    let tavily = Arc::new(TavilyClient::new(tavily_api_key));

    // 创建搜索助手并编译图
    let assistant = SearchAssistant::new(client, tavily);
    let app = Arc::new(assistant.compile());

    println!("{}🔍 智能搜索助手启动！{}", COLOR_CYAN, COLOR_RESET);
    println!("我会使用 Tavily API 为您搜索最新、最准确的信息");
    println!("支持各种问题：新闻、技术、知识问答等");
    println!("(输入 'quit' 退出)\n");

    let mut session_count = 0u32;
    loop {
        // 读取用户输入
        let input = format!("{}🤔 您想了解什么: {}", COLOR_YELLOW, COLOR_RESET);
        let input = read_input_cow(input.as_ref())?.to_string();

        if input.is_empty() {
            continue;
        }

        if matches!(input.to_lowercase().as_str(), "quit" | "q" | "退出" | "exit") {
            println!("感谢使用！再见！👋");
            break;
        }

        session_count += 1;
        let thread_id = format!("session-{}", session_count);

        let initial_state = GraphState::<SearchState> {
            data: SearchState {
                user_query: String::new(),
                search_query: String::new(),
                search_results: String::new(),
                final_answer: String::new(),
                step: "start".into(),
            },
            messages: vec![Message::new("user", &input, Some("用户".into()))],
        };

        println!("\n{}============================================================{}", COLOR_GREEN, COLOR_RESET);

        // ==================== 流式执行 ====================
        let app = Arc::clone(&app);
        let tid = thread_id.clone();

        match app.stream(&tid, initial_state).await {
            Ok(mut rx) => {
                while let Some(event) = rx.recv().await {
                    match event {
                        StreamEvent::NodeStart(name) => {
                            match name.as_str() {
                                "understand" => println!("{}🧠 [节点] 理解阶段开始...{}", COLOR_CYAN, COLOR_RESET),
                                "search" => println!("{}🔍 [节点] 搜索阶段开始...{}", COLOR_YELLOW, COLOR_RESET),
                                "answer" => println!("{}💡 [节点] 生成答案阶段开始...{}", COLOR_GREEN, COLOR_RESET),
                                _ => println!("{}📋 [节点] {} 开始{}", COLOR_CYAN, name, COLOR_RESET),
                            }
                        }
                        StreamEvent::NodeEnd(name, state) => {
                            if name == "answer" {
                                println!("{}✅ 最终答案:{}\n{}", COLOR_GREEN, COLOR_RESET, state.data.final_answer);
                            }
                        }
                        StreamEvent::CheckpointSaved(_, step) => {
                            // 可选：打印检查点保存信息
                            if false { println!("  💾 检查点已保存 (step {})", step); }
                        }
                        StreamEvent::Complete(_) => {}
                    }
                }
            }
            Err(e) => {
                eprintln!("{}❌ 执行错误: {}{}", COLOR_RED, e, COLOR_RESET);
            }
        }

        println!("{}============================================================{}\n", COLOR_GREEN, COLOR_RESET);
    }

    Ok(())
}