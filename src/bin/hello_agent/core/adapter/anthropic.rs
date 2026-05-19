use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;

use super::base::{BaseLLMAdapter, ToolChoice};
use crate::hello_agent::core::exceptions::HelloAgentsError;
use crate::hello_agent::core::message::Message;
use crate::hello_agent::core::response::{LLMResponse, LLMToolResponse, StreamStats, ToolCall};

/// Anthropic Claude 适配器
pub struct AnthropicAdapter {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    timeout: u64,
    last_stats: Mutex<Option<StreamStats>>,
}

impl AnthropicAdapter {
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

    fn messages_url(&self) -> String {
        format!("{}/messages", self.base_url.trim_end_matches('/'))
    }

    fn convert_messages(&self, messages: &[Message]) -> (Option<String>, Vec<serde_json::Value>) {
        let mut system_content = None;
        let mut converted = Vec::new();
        for msg in messages {
            if msg.role.as_str() == "system" {
                system_content = Some(msg.content.clone());
            } else {
                converted.push(serde_json::json!({
                    "role": msg.role.as_str(),
                    "content": msg.content,
                }));
            }
        }
        (system_content, converted)
    }
}

#[async_trait]
impl BaseLLMAdapter for AnthropicAdapter {
    fn last_stats(&self) -> Option<StreamStats> {
        self.last_stats.lock().unwrap().clone()
    }

    async fn invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<LLMResponse, HelloAgentsError> {
        let (system_content, converted) = self.convert_messages(messages);
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": converted,
            "max_tokens": max_tokens.unwrap_or(4096),
        });
        if let Some(t) = temperature {
            body["temperature"] = serde_json::json!(t);
        }
        if let Some(sys) = system_content {
            body["system"] = serde_json::json!(sys);
        }

        let resp = self
            .client
            .post(self.messages_url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| HelloAgentsError::Network(format!("Anthropic请求失败: {}", e)))?;

        let data: serde_json::Value =
            resp.json().await.map_err(|e| HelloAgentsError::Parse(format!("Anthropic解析失败: {}", e)))?;

        let content = data["content"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let mut usage = HashMap::new();
        if let Some(u) = data.get("usage") {
            usage.insert("prompt_tokens".into(), u["input_tokens"].as_u64().unwrap_or(0) as u32);
            usage.insert("completion_tokens".into(), u["output_tokens"].as_u64().unwrap_or(0) as u32);
            usage.insert(
                "total_tokens".into(),
                (u["input_tokens"].as_u64().unwrap_or(0) + u["output_tokens"].as_u64().unwrap_or(0)) as u32,
            );
        }

        Ok(LLMResponse::new(content, self.model.clone(), usage, 0, None))
    }

    async fn stream_invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<mpsc::UnboundedReceiver<String>, HelloAgentsError> {
        let (system_content, converted) = self.convert_messages(messages);
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": converted,
            "max_tokens": max_tokens.unwrap_or(4096),
            "stream": true,
        });
        if let Some(t) = temperature {
            body["temperature"] = serde_json::json!(t);
        }
        if let Some(sys) = system_content {
            body["system"] = serde_json::json!(sys);
        }

        let resp = self
            .client
            .post(self.messages_url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| HelloAgentsError::Network(format!("Anthropic流式请求失败: {}", e)))?;

        let (tx, rx) = mpsc::unbounded_channel();
        let mut stream = resp.bytes_stream();

        tokio::spawn(async move {
            while let Some(chunk) = stream.next().await {
                if let Ok(chunk) = chunk {
                    let text = String::from_utf8_lossy(&chunk);
                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                                if event["type"] == "content_block_delta" {
                                    if let Some(content) = event["delta"]["text"].as_str() {
                                        let _ = tx.send(content.to_string());
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
        Err(HelloAgentsError::LLM("Anthropic工具调用暂未实现".into()))
    }
}