use std::sync::Arc;
use std::time::Duration;
use simple_hello_agents_base::client_message::Message;
use simple_hello_agents_v2::{my_llm, MyLLM};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let llm_client = Arc::new(
    //     reqwest::Client::builder().timeout(Duration::from_secs(180)).build()?,
    // );
    let provider = my_llm::Provider::new(
        Some("deepseek-v4-flash".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None, // Some(llm_client),
    );

    let my_llm = MyLLM::new(None, provider);
    let messages = vec![
        Message {
            role: "system".to_string(),
            content: "你是一个有帮助的助手。".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: "你好，请你介绍下你自己。".to_string(),
        },
    ];

    // 使用流式调用
    println!("--- 流式调用 ---");
    let result = my_llm.chat(messages.clone(), 0.0, true).await?;
    println!("=====\n收集到的完整内容：\n{}", result);

    // 使用非流式调用
    // println!("\n--- 非流式调用 ---");
    // let result2 = my_llm.chat(messages, 0.0, false).await?;
    // println!("=====完整内容：\n{}", result2);

    Ok(())
}