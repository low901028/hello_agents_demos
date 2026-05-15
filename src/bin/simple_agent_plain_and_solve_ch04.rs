mod simple_agent;

use anyhow::{Context, Result};
use dotenvy::dotenv;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::Write;
use crate::simple_agent::simple_agent_client::{HelloAgentsLLM, Message};
use crate::simple_agent::simple_agent_plain_and_solve::PlanAndSolveAgent;

// ==================== 6. 主函数 ====================
#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    // 初始化环境变量（打印警告而非中断）
    if let Err(e) = dotenv() {
        eprintln!("警告：加载 .env 文件时出错: {}", e);
    }

    // 创建两个 LLM 客户端（实际配置相同，但需要两份所有权）
    // 为了避免复杂性，我们创建一个，并用 Arc<Mutex<>> 或直接创建两个实例。
    // 由于 HelloAgentsLLM 包含 reqwest::Client（内部是 Arc），复制配置是简单的。
    let model = "deepseek-v4-flash";
    let api_key = env::var("DEEPSEEK_API_KEY").context("请设置 DEEPSEEK_API_KEY")?;
    let base_url = "https://api.deepseek.com";
    let timeout = env::var("LLM_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);

    let planner_llm =
        HelloAgentsLLM::new(Some(&model), Some(&api_key), Some(&base_url), Some(timeout))?;
    let executor_llm =
        HelloAgentsLLM::new(Some(&model), Some(&api_key), Some(&base_url), Some(timeout))?;

    let agent = PlanAndSolveAgent::new(planner_llm, executor_llm);

    //let question = "一个水果店周一卖出了15个苹果。周二卖出的苹果数量是周一的两倍。周三卖出的数量比周二少了5个。请问这三天总共卖出了多少个苹果？";
    let question = r#"我司目前每日新增数据25-30TB,为了满足日常数据分析，统计，模型训练，AI智能化的需求，需要建设一个满足多模态大数据平台；
           帮我整理一份需求，设计, 实施(包括技术选型，运维成本，用户使用成本等)
    "#;
    agent.run(question).await;

    Ok(())
}