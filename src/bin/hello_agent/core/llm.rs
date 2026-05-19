use std::env;
use std::sync::Mutex;

use tokio::sync::mpsc;

use crate::hello_agent::core::adapter::{create_adapter, base::BaseLLMAdapter};
use crate::hello_agent::core::exceptions::HelloAgentsError;
use crate::hello_agent::core::message::Message;
use crate::hello_agent::core::response::{LLMResponse, LLMToolResponse, StreamStats};
use crate::hello_agent::core::adapter::base::ToolChoice;

/// HelloAgents 统一 LLM 客户端
pub struct HelloAgentsLLM {
    pub model: String,
    pub api_key: String,
    pub base_url: String,
    pub temperature: f64,
    pub max_tokens: Option<u32>,
    pub timeout: u64,
    adapter: Box<dyn BaseLLMAdapter>,
    pub last_call_stats: Mutex<Option<StreamStats>>,
}

impl HelloAgentsLLM {
    /// 创建 LLM 客户端，参数优先级：传入参数 > 环境变量
    pub fn new(
        model: Option<String>,
        api_key: Option<String>,
        base_url: Option<String>,
        temperature: Option<f64>,
        max_tokens: Option<u32>,
        timeout: Option<u64>,
    ) -> Result<Self, HelloAgentsError> {
        dotenvy::dotenv().ok();

        let model = model
            .or_else(|| env::var("LLM_MODEL_ID").ok())
            .ok_or_else(|| HelloAgentsError::Config("必须提供模型名称".into()))?;

        let api_key = api_key
            .or_else(|| env::var("LLM_API_KEY").ok())
            .ok_or_else(|| HelloAgentsError::Config("必须提供API密钥".into()))?;

        let base_url = base_url
            .or_else(|| env::var("LLM_BASE_URL").ok())
            .ok_or_else(|| HelloAgentsError::Config("必须提供服务地址".into()))?;

        let timeout = timeout
            .or_else(|| env::var("LLM_TIMEOUT").ok().and_then(|v| v.parse().ok()))
            .unwrap_or(60);

        let temperature = temperature.unwrap_or(0.7);

        let adapter = create_adapter(api_key.clone(), Some(base_url.clone()), timeout, model.clone());

        Ok(Self {
            model,
            api_key,
            base_url,
            temperature,
            max_tokens,
            timeout,
            adapter,
            last_call_stats: Mutex::new(None),
        })
    }

    /// 获取最后一次流式统计
    pub fn last_call_stats(&self) -> Option<StreamStats> {
        self.last_call_stats.lock().unwrap().clone()
    }

    /// 流式调用（主要方法）
    pub async fn think(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
    ) -> Result<mpsc::UnboundedReceiver<String>, HelloAgentsError> {
        println!("🧠 正在调用 {} 模型...", self.model);

        let temp = temperature.unwrap_or(self.temperature);
        match self.adapter.stream_invoke(messages, Some(temp), self.max_tokens).await {
            Ok(rx) => {
                println!("✅ 大语言模型响应成功:");
                Ok(rx)
            }
            Err(e) => {
                println!("❌ 调用LLM API时发生错误: {}", e);
                Err(e)
            }
        }
    }

    /// 非流式调用
    pub async fn invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<LLMResponse, HelloAgentsError> {
        let temp = temperature.unwrap_or(self.temperature);
        let mt = max_tokens.or(self.max_tokens);
        self.adapter.invoke(messages, Some(temp), mt).await
    }

    /// 流式调用别名
    pub async fn stream_invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
    ) -> Result<mpsc::UnboundedReceiver<String>, HelloAgentsError> {
        self.think(messages, temperature).await
    }

    /// 工具调用
    pub async fn invoke_with_tools(
        &self,
        messages: &[Message],
        tools: &[serde_json::Value],
        tool_choice: &ToolChoice,
        temperature: Option<f64>,
    ) -> Result<LLMToolResponse, HelloAgentsError> {
        let temp = temperature.unwrap_or(self.temperature);
        self.adapter
            .invoke_with_tools(messages, tools, tool_choice, Some(temp), self.max_tokens)
            .await
    }

    /// 异步非流式调用
    pub async fn ainvoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<LLMResponse, HelloAgentsError> {
        self.invoke(messages, temperature, max_tokens).await
    }

    /// 异步流式调用
    pub async fn astream_invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
    ) -> Result<mpsc::UnboundedReceiver<String>, HelloAgentsError> {
        let temp = temperature.unwrap_or(self.temperature);
        self.adapter.astream_invoke(messages, Some(temp), self.max_tokens).await
    }

    /// 异步工具调用
    pub async fn ainvoke_with_tools(
        &self,
        messages: &[Message],
        tools: &[serde_json::Value],
        tool_choice: &ToolChoice,
        temperature: Option<f64>,
    ) -> Result<LLMToolResponse, HelloAgentsError> {
        self.invoke_with_tools(messages, tools, tool_choice, temperature).await
    }
}