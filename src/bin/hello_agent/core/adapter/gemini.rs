use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;

use super::base::{BaseLLMAdapter, ToolChoice};
use crate::hello_agent::core::exceptions::HelloAgentsError;
use crate::hello_agent::core::message::Message;
use crate::hello_agent::core::response::{LLMResponse, LLMToolResponse, StreamStats};

/// Google Gemini 适配器
pub struct GeminiAdapter {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    timeout: u64,
    last_stats: Mutex<Option<StreamStats>>,
}

impl GeminiAdapter {
    pub fn new(api_key: String, base_url: String, model: String, timeout: u64) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout))
            .build()
            .unwrap_or_default();
        Self {
            client,
            api_key,
            base_url,
            model,
            timeout,
            last_stats: Mutex::new(None),
        }
    }

    fn generate_url(&self) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        )
    }

    fn stream_url(&self) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.model, self.api_key
        )
    }

    fn convert_messages(&self, messages: &[Message]) -> Vec<serde_json::Value> {
        messages
            .iter()
            .map(|msg| {
                let role = if msg.role.as_str() == "assistant" { "model" } else { "user" };
                serde_json::json!({
                    "role": role,
                    "parts": [{"text": msg.content}],
                })
            })
            .collect()
    }
}

#[async_trait]
impl BaseLLMAdapter for GeminiAdapter {
    fn last_stats(&self) -> Option<StreamStats> {
        self.last_stats.lock().unwrap().clone()
    }

    async fn invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<LLMResponse, HelloAgentsError> {
        let contents = self.convert_messages(messages);
        let mut body = serde_json::json!({ "contents": contents });

        let mut generation_config = serde_json::json!({});
        if let Some(t) = temperature {
            generation_config["temperature"] = serde_json::json!(t);
        }
        if let Some(mt) = max_tokens {
            generation_config["maxOutputTokens"] = serde_json::json!(mt);
        }
        if !generation_config.as_object().unwrap().is_empty() {
            body["generationConfig"] = generation_config;
        }

        let resp = self
            .client
            .post(self.generate_url())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| HelloAgentsError::Network(format!("Gemini请求失败: {}", e)))?;

        let data: serde_json::Value =
            resp.json().await.map_err(|e| HelloAgentsError::Parse(format!("Gemini解析失败: {}", e)))?;

        let content = data["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let mut usage = HashMap::new();
        if let Some(um) = data.get("usageMetadata") {
            usage.insert("prompt_tokens".into(), um["promptTokenCount"].as_u64().unwrap_or(0) as u32);
            usage.insert("completion_tokens".into(), um["candidatesTokenCount"].as_u64().unwrap_or(0) as u32);
            usage.insert("total_tokens".into(), um["totalTokenCount"].as_u64().unwrap_or(0) as u32);
        }

        Ok(LLMResponse::new(content, self.model.clone(), usage, 0, None))
    }

    async fn stream_invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<mpsc::UnboundedReceiver<String>, HelloAgentsError> {
        let contents = self.convert_messages(messages);
        let mut body = serde_json::json!({ "contents": contents });

        let mut generation_config = serde_json::json!({});
        if let Some(t) = temperature {
            generation_config["temperature"] = serde_json::json!(t);
        }
        if let Some(mt) = max_tokens {
            generation_config["maxOutputTokens"] = serde_json::json!(mt);
        }
        if !generation_config.as_object().unwrap().is_empty() {
            body["generationConfig"] = generation_config;
        }

        let resp = self
            .client
            .post(self.stream_url())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| HelloAgentsError::Network(format!("Gemini流式请求失败: {}", e)))?;

        let (tx, rx) = mpsc::unbounded_channel();
        let mut stream = resp.bytes_stream();

        tokio::spawn(async move {
            while let Some(chunk) = stream.next().await {
                if let Ok(chunk) = chunk {
                    let text = String::from_utf8_lossy(&chunk);
                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                                if let Some(candidates) = event["candidates"].as_array() {
                                    for candidate in candidates {
                                        if let Some(parts) = candidate["content"]["parts"].as_array() {
                                            for part in parts {
                                                if let Some(t) = part["text"].as_str() {
                                                    let _ = tx.send(t.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    async fn invoke_with_tools(
        &self,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _tool_choice: &ToolChoice,
        _temperature: Option<f64>,
        _max_tokens: Option<u32>,
    ) -> Result<LLMToolResponse, HelloAgentsError> {
        Err(HelloAgentsError::LLM("Gemini工具调用暂未实现".into()))
    }
}