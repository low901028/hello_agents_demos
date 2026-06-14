use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, Write};
use futures::StreamExt; // 用于处理 SSE 流

// ---------- 数据结构 ----------
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

// ---------- DeepSeek 客户端 ----------
struct DeepSeekClient {
    api_key: String,
    base_url: String,
    http_client: Client,
}

impl DeepSeekClient {
    /// 创建客户端，优先从环境变量读取 API KEY 和 BASE URL
    fn new() -> Self {
        // 加载 .env 文件（如果存在），失败也不报错
        let _ = dotenvy::dotenv();

        let api_key = env::var("LLM_API_KEY")
            .expect("环境变量 DEEPSEEK_API_KEY 未设置");
        let base_url = env::var("LLM_BASE_URL")
            .unwrap_or_else(|_| "https://api.deepseek.com".to_string());

        Self {
            api_key,
            base_url,
            http_client: Client::new(),
        }
    }

    /// 发送对话请求，返回流式收集的完整响应文本
    async fn chat(
        &self,
        model: &str,           // 如 "deepseek-v4-flash" / "deepseek-v4-pro"
        messages: Vec<Message>,
        temperature: f32,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = ChatRequest {
            model: model.to_string(),
            messages,
            temperature,
            stream: true,       // 强制使用流式（与之前设计一致）
        };

        let resp = self.http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await?;
            return Err(format!("API 请求失败 ({}): {}", status, text).into());
        }

        let mut stream = resp.bytes_stream();
        let mut collected = String::new();
        println!("🤖 模型响应：\n");

        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // 解析 SSE 行 (格式: "test_data: {json}\n\n")
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }
                if let Some(data) = line.strip_prefix("test_data: ") {
                    if data == "[DONE]" {
                        break;
                    }
                    match serde_json::from_str::<ChatChunk>(data) {
                        Ok(chunk_data) => {
                            if let Some(choice) = chunk_data.choices.first() {
                                if let Some(content) = &choice.delta.content {
                                    print!("{}", content);
                                    io::stdout().flush().unwrap(); // 实时输出
                                    collected.push_str(content);
                                }
                            }
                        }
                        Err(e) => eprintln!("\n⚠️  解析流块错误: {}", e),
                    }
                }
            }
        }
        println!();
        Ok(collected)
    }
}

// ---------- 示例用法 ----------
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DeepSeekClient::new();

    // 准备对话历史
    let messages = vec![
        Message {
            role: "system".to_string(),
            content: "你是一个乐于助人的助手，使用中文回答。".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: "请用三句话介绍 Rust 语言。".to_string(),
        },
    ];

    // 选择模型（可以方便地切换）
    let model = "deepseek-v4-flash";  // 或 "deepseek-v4-pro"

    println!("🚀 使用模型: {}\n", model);
    let response = client.chat(model, messages, 0.0).await?;

    println!("\n📝 完整响应:\n{}", response);
    Ok(())
}