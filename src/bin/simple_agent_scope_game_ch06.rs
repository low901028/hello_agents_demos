mod simple_agent;

use anyhow::{Result, Context};
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use crate::simple_agent::simple_agent_client::HelloAgentsLLM;
use crate::simple_agent::simple_agent_scope::simple_agent_scope_game::ThreeKingdomsWerewolfGame;

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

    println!("🎮 欢迎来到三国狼人杀！");

    let client = Arc::new(HelloAgentsLLM::new(
        Some(&model),
        Some(api_key.as_str()),
        Some(base_url.as_str()),
        Some(timeout), // 游戏需要更长超时
    )?);

    // 开始游戏
    let mut game = ThreeKingdomsWerewolfGame::new(client);
    game.run_game(6).await?;

    Ok(())
}