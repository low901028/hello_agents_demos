// src/infra/llm/hello_agents_llm.rs
// HelloAgents 统一 LLM 客户端

use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex, Mutex};

use futures::stream::{self, Stream, StreamExt};
use tokio::task;

use crate::core::traits::adapter::LLMAdapter;
use crate::core::types::exceptions::HelloAgentException;
use crate::core::types::llm_response::{LlmResponse, LlmToolResponse, StreamStats};
use crate::core::types::Message;
use crate::infra::llm::adapters::openai::create_adapter;

#[derive(Clone)]
pub struct HelloAgentsLLM {
    pub model: String,
    pub api_key: String,
    pub base_url: String,
    pub temperature: f32,
    pub max_tokens: Option<usize>,
    pub timeout: usize,
    pub adapter: Arc<dyn LLMAdapter>,
    pub last_call_stats: Arc<Mutex<Option<StreamStats>>>,
    pub kwargs: Option<HashMap<String, String>>,
}

impl HelloAgentsLLM {
    pub fn new(
        model: Option<&str>,
        api_key: Option<&str>,
        base_url: Option<&str>,
        temperature: Option<f32>,
        max_tokens: Option<usize>,
        timeout: Option<usize>,
        kwargs: Option<HashMap<String, String>>,
    ) -> Result<Self, HelloAgentException> {
        let model = model
            .map(|s| s.to_string())
            .or_else(|| env::var("LLM_MODEL_ID").ok())
            .ok_or_else(|| HelloAgentException::config("必须提供模型名称 (model 或 LLM_MODEL_ID)"))?;
        let api_key = api_key
            .map(|s| s.to_string())
            .or_else(|| env::var("LLM_API_KEY").ok())
            .ok_or_else(|| HelloAgentException::config("必须提供API密钥 (api_key 或 LLM_API_KEY)"))?;
        let base_url = base_url
            .map(|s| s.to_string())
            .or_else(|| env::var("LLM_BASE_URL").ok())
            .ok_or_else(|| HelloAgentException::config("必须提供服务地址 (base_url 或 LLM_BASE_URL)"))?;
        let timeout = timeout.unwrap_or_else(|| {
            env::var("LLM_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60)
        });
        let temperature = temperature.unwrap_or_else(|| {
            env::var("LLM_TEMPERATURE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.7)
        });

        let adapter = create_adapter(&api_key, &base_url, timeout as i32, &model)?;

        Ok(HelloAgentsLLM {
            model,
            api_key,
            base_url,
            temperature,
            max_tokens,
            timeout,
            adapter,
            last_call_stats: Arc::new(StdMutex::new(None)),
            kwargs,
        })
    }

    pub fn adapter(&self) -> Arc<dyn LLMAdapter> {
        Arc::clone(&self.adapter)
    }

    pub fn think(
        &self,
        messages: Vec<Message>,
        temperature: Option<f32>,
    ) -> impl Iterator<Item = Result<String, HelloAgentException>> + Send + 'static {
        println!("🧠 正在调用 {} 模型...", self.model);

        let mut kwargs = HashMap::new();
        kwargs.insert(
            "temperature".to_string(),
            temperature.unwrap_or(self.temperature).to_string(),
        );
        if let Some(max_tokens) = self.max_tokens {
            kwargs.insert("max_tokens".to_string(), max_tokens.to_string());
        }

        println!("✅ 大语言模型响应成功:");

        let stream_result = self.adapter.stream_invoke(messages, Some(kwargs));
        let mut iter: Option<
            Box<dyn Iterator<Item = Result<LlmResponse, HelloAgentException>> + Send>,
        > = None;
        let mut init_err: Option<HelloAgentException> = None;
        match stream_result {
            Ok(i) => iter = Some(i),
            Err(e) => init_err = Some(e),
        }

        let stats_arc = self.last_call_stats.clone();
        let adapter_clone = self.adapter.clone();

        std::iter::from_fn(move || {
            if let Some(e) = init_err.take() {
                return Some(Err(e));
            }
            let iter = iter.as_mut().unwrap();
            loop {
                match iter.next() {
                    Some(Ok(llm_resp)) => {
                        print!("{}", llm_resp.content);
                        let _ = io::stdout().flush();
                        return Some(Ok(llm_resp.content));
                    }
                    Some(Err(e)) => return Some(Err(e)),
                    None => {
                        println!();
                        if let Some(stats) = adapter_clone.last_stats() {
                            if let Ok(mut guard) = stats_arc.lock() {
                                *guard = Some(stats);
                            }
                        }
                        return None;
                    }
                }
            }
        })
    }

    pub fn invoke(
        &self,
        messages: Vec<Message>,
    ) -> Result<LlmResponse, HelloAgentException> {
        let mut kwargs = HashMap::new();
        kwargs.insert("temperature".to_string(), self.temperature.to_string());
        if let Some(max_tokens) = self.max_tokens {
            kwargs.insert("max_tokens".to_string(), max_tokens.to_string());
        }
        self.adapter.invoke(messages, kwargs)
    }

    pub fn stream_invoke(
        &self,
        messages: Vec<Message>,
    ) -> Result<
        Box<dyn Iterator<Item = Result<String, HelloAgentException>> + Send>,
        HelloAgentException,
    > {
        let iter = self.think(messages, None);
        Ok(Box::new(iter))
    }

    pub fn invoke_with_tools(
        &self,
        messages: Vec<Message>,
        tools: Vec<HashMap<String, serde_json::Value>>,
        tool_choice: Option<String>,
    ) -> Result<LlmToolResponse, HelloAgentException> {
        let mut kwargs = HashMap::new();
        kwargs.insert(
            "temperature".to_string(),
            serde_json::Value::String(self.temperature.to_string()),
        );
        if let Some(tc) = tool_choice {
            kwargs.insert("tool_choice".to_string(), serde_json::Value::String(tc));
        }
        if let Some(max_tokens) = self.max_tokens {
            kwargs.insert(
                "max_tokens".to_string(),
                serde_json::Value::String(max_tokens.to_string()),
            );
        }
        self.adapter.invoke_with_tools(messages, tools, kwargs)
    }

    // ================= 异步方法 =================
    pub async fn ainvoke(
        &self,
        messages: Vec<Message>,
    ) -> Result<LlmResponse, HelloAgentException> {
        let adapter = self.adapter.clone();
        let temperature = self.temperature;
        let max_tokens = self.max_tokens;
        task::spawn_blocking(move || {
            let mut kwargs = HashMap::new();
            kwargs.insert("temperature".to_string(), temperature.to_string());
            if let Some(mt) = max_tokens {
                kwargs.insert("max_tokens".to_string(), mt.to_string());
            }
            adapter.invoke(messages, kwargs)
        })
            .await
            .map_err(|e| HelloAgentException::llm(format!("异步任务失败: {}", e)))?
    }

    pub async fn astream_invoke(
        &self,
        messages: Vec<Message>,
    ) -> Pin<Box<dyn Stream<Item = Result<String, HelloAgentException>> + Send>> {
        let mut kwargs = HashMap::new();
        kwargs.insert("temperature".to_string(), self.temperature.to_string());
        if let Some(mt) = self.max_tokens {
            kwargs.insert("max_tokens".to_string(), mt.to_string());
        }

        let base_stream = self
            .adapter
            .astream_invoke(messages, Some(kwargs))
            .await;
        let stats_arc = self.last_call_stats.clone();
        let adapter_clone = self.adapter.clone();

        let update_stream = stream::once(async move {
            if let Some(stats) = adapter_clone.last_stats() {
                if let Ok(mut guard) = stats_arc.lock() {
                    *guard = Some(stats);
                }
            }
            Err(HelloAgentException::StreamEnd)
        });

        let combined = base_stream
            .chain(update_stream)
            .filter_map(|res| async move {
                match res {
                    Ok(s) => Some(Ok(s)),
                    Err(HelloAgentException::StreamEnd) => None,
                    Err(e) => Some(Err(e)),
                }
            });
        Box::pin(combined)
    }

    pub async fn ainvoke_with_tools(
        &self,
        messages: Vec<Message>,
        tools: Vec<HashMap<String, serde_json::Value>>,
        tool_choice: Option<String>,
    ) -> Result<LlmToolResponse, HelloAgentException> {
        let adapter = self.adapter.clone();
        let temperature = self.temperature;
        let max_tokens = self.max_tokens;
        task::spawn_blocking(move || {
            let mut kwargs = HashMap::new();
            kwargs.insert(
                "temperature".to_string(),
                serde_json::Value::String(temperature.to_string()),
            );
            if let Some(tc) = tool_choice {
                kwargs.insert("tool_choice".to_string(), serde_json::Value::String(tc));
            }
            if let Some(mt) = max_tokens {
                kwargs.insert(
                    "max_tokens".to_string(),
                    serde_json::Value::String(mt.to_string()),
                );
            }
            adapter.invoke_with_tools(messages, tools, kwargs)
        })
            .await
            .map_err(|e| HelloAgentException::llm(format!("异步任务失败: {}", e)))?
    }

    pub fn update_last_stats(&self) {
        if let Some(stats) = self.adapter.last_stats() {
            if let Ok(mut guard) = self.last_call_stats.lock() {
                *guard = Some(stats);
            }
        }
    }

    pub fn get_last_stats(&self) -> Option<StreamStats> {
        self.last_call_stats.lock().ok().and_then(|g| g.clone())
    }
}