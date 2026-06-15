// ============================================================
// src/infra/openai_adapter.rs
// ============================================================
use crate::core::traits::llm_provider::LlmProvider;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::llm_response::{LlmResponse, StreamChunk};
use crate::core::types::message::{Message, ToolDefinition};
use async_trait::async_trait;
use futures::stream::Stream;
use reqwest::Client;
use std::pin::Pin;

pub struct OpenAIAdapter {
    api_key: String,
    base_url: String,
    model: String,
    client: Client,
}

impl OpenAIAdapter {
    pub fn new(api_key: &str, base_url: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            base_url: base_url.to_string(),
            model: model.to_string(),
            client: Client::new(),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAIAdapter {
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&str>,
    ) -> Result<LlmResponse, HelloAgentError> {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
        });
        if let Some(t) = tools {
            body["tools"] = serde_json::to_value(t)?;
        }
        if let Some(tc) = tool_choice {
            body["tool_choice"] = serde_json::to_value(tc)?;
        }

        //println!("request body={:?}", serde_json::to_string_pretty(&body)?);

        let resp = self
            .client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(HelloAgentError::General(format!(
                "API error {}",
                resp.status()
            )));
        }

        let json: serde_json::Value = resp.json().await?;
        let choices = json["choices"]
            .as_array()
            .ok_or(HelloAgentError::General("missing choices".into()))?;
        let msg = &choices[0]["message"];
        let content = msg["content"].as_str().map(String::from);
        let tool_calls: Vec<_> =
            serde_json::from_value(msg["tool_calls"].clone()).unwrap_or_default();
        let usage: crate::core::types::message::Usage =
            serde_json::from_value(json["usage"].clone()).unwrap_or_default();
        Ok(LlmResponse {
            content,
            tool_calls,
            usage,
            model: json["model"].as_str().unwrap_or(&self.model).into(),
        })
    }

    async fn chat_stream(
        &self,
        _messages: &[Message],
        _tools: Option<&[ToolDefinition]>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<StreamChunk, HelloAgentError>> + Send>>,
        HelloAgentError,
    > {
        Err(HelloAgentError::General("streaming not implemented".into()))
    }
}
