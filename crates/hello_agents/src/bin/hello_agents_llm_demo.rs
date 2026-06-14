use anyhow::{Context, Result};
use futures::StreamExt;

use std::collections::HashMap;
use hello_agents::core::types::Message;
use hello_agents::core::types::message::{MessageContent, MessageRole};
use hello_agents::infra::llm::hello_agents_llm::HelloAgentsLLM;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let llm = HelloAgentsLLM::new(
        Some("deepseek-v4-pro"),
        None,
        None,
        Some(0.7f32),
        Some(393216),
        Some(180usize),
        None,
    )?;

    let messages = vec![
        Message {
            role: MessageRole::System,
            content: Some(MessageContent::Text("你是一个有帮助的助手。".to_string())),
            tool_call_id: None,
            tool_calls: None,
            name: None,
            extra: Default::default(),
        },
        Message {
            role: MessageRole::User,
            content: Some(MessageContent::Text("你好，请你介绍下你自己。".to_string())),
            tool_call_id: None,
            tool_calls: None,
            name: None,
            extra: Default::default(),
        },
    ];

    // ainvoke
    let resp = llm.ainvoke(messages.clone()).await?;
    assert!(!resp.content.is_empty());
    println!("ainvoke 响应: {}", resp.content);

    // astream_invoke
    let mut stream = llm.astream_invoke(messages.clone()).await;
    let mut chunks = Vec::new();
    while let Some(Ok(chunk)) = stream.next().await {
        chunks.push(chunk);
    }
    let full_text: String = chunks.concat();
    println!("astream_invoke 结果: {:?}", full_text);

    // think模式
    let llm = std::sync::Arc::new(llm);
    let llm_clone = llm.clone();
    let chunks = tokio::task::spawn_blocking(move || {
        let mut result = Vec::new();
        for chunk in llm_clone.think(messages, None) {
            match chunk {
                Ok(t) => result.push(t),
                Err(e) => return Err(e),
            }
        }
        Ok(result)
    })
    .await?
    .expect("流式错误");

    let full = chunks.concat();
    println!("think 结果: {}", full);
    // 流结束后应自动保存统计
    let saved = llm.get_last_stats();
    if let Some(stats) = saved {
        println!("get_last_stats: {:?}", stats);
    } else {
        println!("get_last_stats None");
    }

    Ok(())
}
