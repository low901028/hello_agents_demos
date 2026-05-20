use crate::hello_agent::core::exceptions::HelloAgentsError;
use crate::hello_agent::core::llm_adapters::{BaseLlmAdapter, create_adapter};
use crate::hello_agent::core::llm_response::{LlmResponse, LlmToolResponse, StreamStats};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use dotenvy::dotenv;

#[derive(Clone)]
pub struct HelloAgentsLlm {
    pub model: String,
    temperature: f64,
    max_tokens: Option<usize>,
    timeout: usize,
    adapter: Arc<dyn BaseLlmAdapter>,
    pub last_call_stats: Arc<RwLock<Option<StreamStats>>>,
}

impl HelloAgentsLlm {
    pub fn new(
        model: Option<&str>,
        api_key: Option<&str>,
        base_url: Option<&str>,
        temperature: f64,
        max_tokens: Option<usize>,
        timeout: Option<usize>,
    ) -> Result<Self, HelloAgentsError> {
        dotenv().ok();

        let model = model
            .map(|s| s.to_string())
            .or_else(|| env::var("LLM_MODEL_ID").ok())
            .ok_or_else(|| HelloAgentsError::config("必须提供模型名称"))?;
        let api_key = api_key
            .map(|s| s.to_string())
            .or_else(|| env::var("LLM_API_KEY").ok())
            .ok_or_else(|| HelloAgentsError::config("必须提供API密钥"))?;
        let base_url = base_url
            .map(|s| s.to_string())
            .or_else(|| env::var("LLM_BASE_URL").ok())
            .ok_or_else(|| HelloAgentsError::config("必须提供服务地址"))?;
        let timeout = timeout.unwrap_or_else(|| {
            env::var("LLM_TIMEOUT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(60)
        });
        if model.is_empty() || api_key.is_empty() {
            return Err(HelloAgentsError::config("模型名称和API密钥不能为空"));
        }
        let adapter = create_adapter(&api_key, &base_url, timeout, &model)?;
        Ok(HelloAgentsLlm {
            model,
            temperature,
            max_tokens,
            timeout,
            adapter: adapter.into(),
            last_call_stats: Arc::new(RwLock::new(None)),
        })
    }

    pub fn invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
    ) -> Result<LlmResponse, HelloAgentsError> {
        self.adapter
            .invoke(messages, self.temperature, self.max_tokens)
    }

    pub fn stream_invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
    ) -> Result<Box<dyn Iterator<Item = Result<String, HelloAgentsError>> + Send>, HelloAgentsError>
    {
        self.adapter
            .stream_invoke(messages, self.temperature, self.max_tokens)
    }

    pub async fn astream_invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, HelloAgentsError>>, HelloAgentsError>
    {
        self.adapter
            .astream_invoke(messages, self.temperature, self.max_tokens)
            .await
    }

    pub async fn ainvoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
    ) -> Result<LlmResponse, HelloAgentsError> {
        self.adapter
            .ainvoke(messages, self.temperature, self.max_tokens)
            .await
    }

    pub fn invoke_with_tools(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        tools: &[HashMap<String, serde_json::Value>],
        tool_choice: &str,
    ) -> Result<LlmToolResponse, HelloAgentsError> {
        self.adapter.invoke_with_tools(
            messages,
            tools,
            tool_choice,
            self.temperature,
            self.max_tokens,
        )
    }

    pub fn get_last_stats(&self) -> Option<StreamStats> {
        self.last_call_stats.read().clone()
    }
}
