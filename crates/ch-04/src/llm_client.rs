use reqwest::Client;
use std::env;
use anyhow::Context;
use futures::StreamExt;
use crate::client_message::{ChatChunk, ChatRequest, Message};

#[derive(Clone)]   // ← 新增 Clone 派生
pub struct LLMClient {
    model: String,
    api_key: String,
    base_url: String,
    http_client: Client,
}

impl LLMClient {
    pub fn new(
        model: Option<String>,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> Self {
        let _ = dotenvy::dotenv();

        let model = model
            .or_else(|| env::var("LLM_MODEL_ID").ok())
            .unwrap_or_else(|| "deepseek-chat".to_string());
        let api_key = api_key
            .or_else(|| env::var("LLM_API_KEY").ok())
            .expect("API key must be provided or set in LLM_API_KEY env var");
        let base_url = base_url
            .or_else(|| env::var("LLM_BASE_URL").ok())
            .unwrap_or_else(|| "https://api.deepseek.com/v1".to_string()); // 注意补充 /v1

        let http_client = Client::new();

        LLMClient {
            model,
            api_key,
            base_url,
            http_client,
        }
    }

    pub async fn think(
        &self,
        messages: Vec<Message>,
        temperature: f32,
    ) -> Result<String, Box<dyn std::error::Error>> {
        println!("🧠 正在调用 {} 模型...", self.model);

        let url = format!("{}/chat/completions", self.base_url);
        let request_body = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature,
            stream: true,
        };

        println!("request body: {:?}", serde_json::to_string(&request_body));
        let response = self.http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("LLM API 请求失败")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(format!("API returned {status}: {text}").into());
        }

        let mut stream = response.bytes_stream();
        let mut collected = String::new();
        println!("✅ 大语言模型响应成功:");

        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        break;
                    }
                    match serde_json::from_str::<ChatChunk>(data) {
                        Ok(chunk_data) => {
                            if let Some(choice) = chunk_data.choices.first() {
                                if let Some(content) = &choice.delta.content {
                                    print!("{}", content);
                                    collected.push_str(content);
                                }
                            }
                        }
                        Err(e) => eprintln!("无法解析流块: {e}"),
                    }
                }
            }
        }
        println!();
        Ok(collected)
    }
}