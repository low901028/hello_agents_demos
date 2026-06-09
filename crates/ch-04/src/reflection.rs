use crate::llm_client::{LLMClient};
use crate::client_message::{Message};

const REFLECTION_PROMPT_TEMPLATE: &str = r#"
你是一位资深的Rust程序员。你正在根据一位代码评审专家的反馈来优化你的代码。

# 原始任务:
编写一个Rust函数，找出1到n之间所有的素数 (prime numbers)。

# 你上一轮尝试的代码:
{original_code}

# 评审员的反馈:
{feedback}

请根据评审员的反馈，生成一个优化后的新版本代码。
你的代码必须包含完整的函数签名、文档字符串，并遵循PEP 8编码规范。
请直接输出优化后的代码，不要包含任何额外的解释。
"#;

pub async fn reflect_and_optimize(
    llm_client: &LLMClient,
    original_code: &str,
    feedback: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let prompt = REFLECTION_PROMPT_TEMPLATE
        .replace("{original_code}", original_code)
        .replace("{feedback}", feedback);

    let messages = vec![Message {
        role: "user".to_string(),
        content: prompt,
    }];

    println!("--- 反思优化中 ---");
    let optimized_code = llm_client.think(messages, 0.0).await?;
    Ok(optimized_code)
}