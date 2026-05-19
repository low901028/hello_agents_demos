use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::mpsc;

use super::base::{BaseLLMAdapter, ToolChoice};
use crate::hello_agent::core::exceptions::HelloAgentsError;
use crate::hello_agent::core::message::Message;
use crate::hello_agent::core::response::{LLMResponse, LLMToolResponse, StreamStats, ToolCall};

/// =================================================================
/// OpenAI 兼容适配器
/// 支持openai协议的均可通过该adapter来完成请求
/// 
/// =================================================================
pub struct OpenAIAdapter {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    timeout: u64,
    last_stats: Mutex<Option<StreamStats>>,
}

impl OpenAIAdapter {
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

    fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    fn build_body(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
        stream: bool,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages.iter().map(|m| m.to_dict()).collect::<Vec<_>>(),
            "stream": stream,
        });
        if let Some(t) = temperature {
            body["temperature"] = serde_json::json!(t);
        }
        if let Some(mt) = max_tokens {
            body["max_tokens"] = serde_json::json!(mt);
        }
        body
    }

    fn parse_usage(data: &serde_json::Value) -> HashMap<String, u32> {
        let mut usage = HashMap::new();
        if let Some(u) = data.get("usage") {
            usage.insert("prompt_tokens".into(), u["prompt_tokens"].as_u64().unwrap_or(0) as u32);
            usage.insert("completion_tokens".into(), u["completion_tokens"].as_u64().unwrap_or(0) as u32);
            usage.insert("total_tokens".into(), u["total_tokens"].as_u64().unwrap_or(0) as u32);
        }
        usage
    }
}

#[async_trait]
impl BaseLLMAdapter for OpenAIAdapter {
    fn last_stats(&self) -> Option<StreamStats> {
        self.last_stats.lock().unwrap().clone()
    }

    async fn invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<LLMResponse, HelloAgentsError> {
        let body = self.build_body(messages, temperature, max_tokens, false);
        let start = Instant::now();

        let resp = self
            .client
            .post(self.chat_url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| HelloAgentsError::Network(format!("请求失败: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(HelloAgentsError::LLM(format!("API错误 ({}): {}", status, text)));
        }

        let data: serde_json::Value =
            resp.json().await.map_err(|e| HelloAgentsError::Parse(format!("解析失败: {}", e)))?;

        let choice = &data["choices"][0];
        let content = choice["message"]["content"].as_str().unwrap_or("").to_string();
        let latency_ms = start.elapsed().as_millis() as u64;

        let reasoning_content = choice["message"]
            .get("reasoning_content")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let usage = Self::parse_usage(&data);

        Ok(LLMResponse::new(content, self.model.clone(), usage, latency_ms, reasoning_content))
    }

    async fn stream_invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<mpsc::UnboundedReceiver<String>, HelloAgentsError> {
        let body = self.build_body(messages, temperature, max_tokens, true);

        let resp = self
            .client
            .post(self.chat_url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| HelloAgentsError::Network(format!("流式请求失败: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(HelloAgentsError::LLM(format!("API错误 ({}): {}", status, text)));
        }

        let (tx, rx) = mpsc::unbounded_channel();
        let mut stream = resp.bytes_stream();

        tokio::spawn(async move {
            while let Some(chunk) = stream.next().await {
                if let Ok(chunk) = chunk {
                    let text = String::from_utf8_lossy(&chunk);
                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                break;
                            }
                            if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(data) {
                                if let Some(choices) = chunk["choices"].as_array() {
                                    for choice in choices {
                                        if let Some(content) = choice["delta"]["content"].as_str() {
                                            let _ = tx.send(content.to_string());
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
        messages: &[Message],
        tools: &[serde_json::Value],
        tool_choice: &ToolChoice,
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<LLMToolResponse, HelloAgentsError> {
        let mut body = self.build_body(messages, temperature, max_tokens, false);
        body["tools"] = serde_json::json!(tools);
        body["tool_choice"] = tool_choice.to_json();

        let resp = self
            .client
            .post(self.chat_url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| HelloAgentsError::Network(format!("工具调用请求失败: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(HelloAgentsError::LLM(format!("API错误 ({}): {}", status, text)));
        }

        let data: serde_json::Value =
            resp.json().await.map_err(|e| HelloAgentsError::Parse(format!("解析失败: {}", e)))?;

        let message = &data["choices"][0]["message"];
        let content = message["content"].as_str().map(|s| s.to_string());
        let model = data["model"].as_str().unwrap_or(&self.model).to_string();
        let usage = Self::parse_usage(&data);
        let latency_ms = 0;

        let tool_calls: Vec<ToolCall> = message["tool_calls"]
            .as_array()
            .map(|calls| {
                calls
                    .iter()
                    .map(|tc| ToolCall {
                        id: tc["id"].as_str().unwrap_or("").to_string(),
                        name: tc["function"]["name"].as_str().unwrap_or("").to_string(),
                        arguments: tc["function"]["arguments"].as_str().unwrap_or("{}").to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(LLMToolResponse {
            content,
            tool_calls,
            model,
            usage,
            latency_ms,
        })
    }
}