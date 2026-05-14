/// =========================================
///   基于thought - action - observation模式
/// =========================================
mod simple_agent;
use anyhow::{Context, Result};
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use crate::simple_agent::simple_agent_client::{HelloAgentsLLM};
use crate::simple_agent::simple_agent_react::ReActAgent;
use crate::simple_agent::simple_agent_tools_search::{BaiduSearchClient};
use crate::simple_agent::simple_agent_utils_register::{ToolExecutor};


#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    /// step-1 LLM client构建
    /// step-2 检索工具构建
    /// step-3 agent构建
    /// step-4 向agent发送请求 LLM不能处理，通过检索工具处理，再将内容“喂给”LLM；
    ///        通过不断的观测结果 再修正输出，最终让LLM输出最优解
    // =========================== LLM 构建 ========================= //
    let api_key = env::var("DEEPSEEK_API_KEY").context("请设置 DEEPSEEK_API_KEY")?;
    let model = "deepseek-v4-flash";
    let base_url = "https://api.deepseek.com";
    let timeout = 60u64;

    // 尝试从环境变量/.env 创建客户端
    let llm_client = HelloAgentsLLM::new(
        Some(model),    // model
        Some(&api_key), // api_key
        Some(base_url), // base_url
        Some(timeout),  // timeout
    ).context("创建 LLM 客户端失败")?;

    /// 目前仅使用百度千帆搜索引擎
    // =========================== 检索工具构建 ========================= //
    let api_key = env::var("BAIDU_API_KEY").context("请设置 BAIDU_API_KEY")?;

    let client = Arc::new(BaiduSearchClient::new(api_key));

    let mut executor = ToolExecutor::new();
    // 注册工具
    executor.register_tool(
        "Search".into(),
        "一个网页搜索引擎。当你需要回答关于时事、事实以及在你的知识库中找不到的信息时，应使用此工具。".into(),
        Box::new(crate::simple_agent::simple_agent_tools_search::SearchTool {
            client: client.clone(),
        }),
    );

    // ================================ agent 构建 ================================ //
    // 创建 ReAct Agent
    let mut agent = ReActAgent::new(llm_client, executor, 5);

    // ============================== 测试 =================================//
    let question = "华为最新的手机是哪一款？它的主要卖点是什么？";
    let answer = agent.run(question).await;

    match answer {
        Some(ans) => println!("\n✅ 最终结果:\n{}", ans),
        None => println!("⚠️ 未能获取到最终答案。"),
    }

    Ok(())
}