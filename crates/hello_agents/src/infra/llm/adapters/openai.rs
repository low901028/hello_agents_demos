use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;

use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use serde_json::Value;

use crate::core::traits::adapter::LLMAdapter;
use crate::core::types::exceptions::HelloAgentException;
use crate::core::types::llm_resp_req::*;
use crate::core::types::llm_response::*;
use crate::core::types::Message;

pub struct OpenAIAdapter {
    pub api_key: String,
    pub base_url: String,
    pub timeout: i32,
    pub model: String,
    pub sync_client: Option<reqwest::blocking::Client>,
    pub async_client: Option<reqwest::Client>,
    pub last_stats: Arc<StdMutex<Option<StreamStats>>>,
}

impl OpenAIAdapter {
    pub fn new(api_key: &str, base_url: &str, timeout: i32, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            base_url: base_url.to_string(),
            timeout,
            model: model.to_string(),
            sync_client: None,
            async_client: None,
            last_stats: Arc::new(StdMutex::new(None)),
        }
    }

    fn is_thinking_model(model: &str) -> bool {
        ["reasoner", "o1", "o3", "thinking"]
            .iter()
            .any(|k| model.to_lowercase().contains(k))
    }

    fn build_request_body(
        &self,
        messages: Vec<Message>,
        kwargs: HashMap<String, Value>,
        stream: bool,
        tools: Option<Vec<HashMap<String, Value>>>,
        tool_choice: Option<String>,
    ) -> Value {
        let mut request = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            stream: Some(stream),
            ..Default::default()
        };

        for (k, v) in kwargs {
            match k.as_str() {
                "temperature" => request.temperature = v.as_f64(),
                "max_tokens" => request.max_tokens = v.as_u64().map(|n| n as u32),
                _ => {
                    request.extra.insert(k, v);
                }
            }
        }

        if let Some(tools) = tools {
            let tool_defs: Vec<ToolDefinition> = tools
                .into_iter()
                .map(|mut tool| {
                    let function = tool.remove("function").unwrap_or_default();
                    ToolDefinition {
                        tool_type: "function".to_string(),
                        function: serde_json::from_value(function).unwrap(),
                    }
                })
                .collect();
            request.tools = Some(tool_defs);
        }
        if let Some(tc) = tool_choice {
            request.tool_choice = Some(ToolChoice::String(tc));
        }

        serde_json::to_value(request).unwrap_or(Value::Null)
    }

    fn extract_llm_response(
        json: &Value,
        latency_ms: i64,
        model: &str,
    ) -> Result<LlmResponse, HelloAgentException> {
        let choices = json["choices"]
            .as_array()
            .ok_or_else(|| HelloAgentException::llm("缺少 choices"))?;
        let choice = choices
            .first()
            .ok_or_else(|| HelloAgentException::llm("choices 为空"))?;
        let message = &choice["message"];
        let content = message["content"].as_str().unwrap_or("").to_string();

        let reasoning = if Self::is_thinking_model(model) {
            choice["reasoning_content"]
                .as_str()
                .or_else(|| message["reasoning_content"].as_str())
                .unwrap_or("")
                .to_string()
        } else {
            message["reasoning_content"]
                .as_str()
                .unwrap_or("")
                .to_string()
        };

        let usage: Option<Usage> = serde_json::from_value(json["usage"].clone()).ok();

        Ok(LlmResponse {
            content,
            model: model.into(),
            usage,
            latency_ms,
            reasoning_content: if reasoning.is_empty() {
                None
            } else {
                Some(reasoning)
            },
            backward_compatibility: false,
        })
    }
}

#[async_trait]
impl LLMAdapter for OpenAIAdapter {
    fn create_sync_client(&self) -> Result<reqwest::blocking::Client, HelloAgentException> {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(self.timeout as u64))
            .build()
            .map_err(|e| HelloAgentException::NetworkException(e.to_string()))
    }

    fn create_async_client(&self) -> Result<reqwest::Client, HelloAgentException> {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.timeout as u64))
            .build()
            .map_err(|e| HelloAgentException::NetworkException(e.to_string()))
    }

    fn invoke(
        &self,
        messages: Vec<Message>,
        kwargs: HashMap<String, String>,
    ) -> Result<LlmResponse, HelloAgentException> {
        let client = self
            .sync_client
            .clone()
            .map(Ok)
            .unwrap_or_else(|| self.create_sync_client())?;
        let start = Instant::now();

        let mut value_kwargs = HashMap::new();
        for (k, v) in kwargs {
            let val: Value = if let Ok(num) = v.parse::<f64>() {
                Value::Number(serde_json::Number::from_f64(num).unwrap())
            } else {
                Value::String(v)
            };
            value_kwargs.insert(k, val);
        }

        let body = self.build_request_body(messages, value_kwargs, false, None, None);
        let url = format!("{}/chat/completions", self.base_url);

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(HelloAgentException::llm(format!("API 错误 ({}): {}", status, text)));
        }

        let latency_ms = start.elapsed().as_millis() as i64;
        let json: Value = resp.json()?;
        Self::extract_llm_response(&json, latency_ms, &self.model)
    }

    fn stream_invoke(
        &self,
        messages: Vec<Message>,
        kwargs: Option<HashMap<String, String>>,
    ) -> Result<
        Box<dyn Iterator<Item = Result<LlmResponse, HelloAgentException>> + Send>,
        HelloAgentException,
    > {
        let client = self
            .sync_client
            .clone()
            .map(Ok)
            .unwrap_or_else(|| self.create_sync_client())?;
        let start = Instant::now();

        let mut value_kwargs = HashMap::new();
        if let Some(kw) = kwargs {
            for (k, v) in kw {
                let val: Value = if let Ok(num) = v.parse::<f64>() {
                    Value::Number(serde_json::Number::from_f64(num).unwrap())
                } else {
                    Value::String(v)
                };
                value_kwargs.insert(k, val);
            }
        }

        let body = self.build_request_body(messages, value_kwargs, true, None, None);
        let url = format!("{}/chat/completions", self.base_url);

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&body)
            .send()?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(HelloAgentException::llm(format!("API 错误 ({}): {}", status, text)));
        }

        let text = resp.text()?;
        let mut chunks: Vec<LlmResponse> = Vec::new();
        let mut usage_opt: Option<Usage> = None;
        let mut reasoning_buf = String::new();

        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line["data: ".len()..];
            if data == "[DONE]" {
                continue;
            }
            let json: Value = serde_json::from_str(data)
                .map_err(|e| HelloAgentException::from(e))?;

            if let Some(delta) = json["choices"].get(0).and_then(|c| c.get("delta")) {
                let content = delta
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let reasoning = delta
                    .get("reasoning_content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !reasoning.is_empty() {
                    reasoning_buf.push_str(&reasoning);
                }
                chunks.push(LlmResponse {
                    content,
                    model: self.model.clone(),
                    usage: None,
                    latency_ms: 0,
                    reasoning_content: if reasoning.is_empty() {
                        None
                    } else {
                        Some(reasoning)
                    },
                    backward_compatibility: false,
                });
            }

            if let Some(u) = json.get("usage") {
                usage_opt = serde_json::from_value(u.clone()).ok();
            }
        }

        let latency_ms = start.elapsed().as_millis() as i64;
        let usage = usage_opt.unwrap_or_default();
        *self.last_stats.lock().unwrap() = Some(StreamStats {
            model: self.model.clone(),
            usage,
            latency_ms,
            reasoning_content: if reasoning_buf.is_empty() {
                None
            } else {
                Some(reasoning_buf)
            },
        });

        Ok(Box::new(chunks.into_iter().map(Ok)))
    }

    async fn astream_invoke(
        &self,
        messages: Vec<Message>,
        kwargs: Option<HashMap<String, String>>,
    ) -> Pin<Box<dyn Stream<Item = Result<String, HelloAgentException>> + Send>> {
        let async_client = match self.async_client.clone() {
            Some(c) => Ok(c),
            None => self.create_async_client(),
        };
        let start = Instant::now();

        let mut value_kwargs = HashMap::new();
        if let Some(kw) = kwargs {
            for (k, v) in kw {
                let val: Value = if let Ok(num) = v.parse::<f64>() {
                    Value::Number(serde_json::Number::from_f64(num).unwrap())
                } else {
                    Value::String(v)
                };
                value_kwargs.insert(k, val);
            }
        }

        let body = self.build_request_body(messages, value_kwargs, true, None, None);
        let url = format!("{}/chat/completions", self.base_url);
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let last_stats = self.last_stats.clone();

        let stream = async_stream::try_stream! {
            let client = async_client?;
            let resp = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .header("Accept", "text/event-stream")
                .json(&body)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                Err(HelloAgentException::llm(format!("API 错误 ({}): {}", status, "LLM获取api数据异常")))?;
            }

            let mut stream = resp.bytes_stream();
            let mut usage_opt: Option<Usage> = None;
            let mut reasoning_buf = String::new();

            while let Some(bytes) = stream.next().await {
                let bytes = bytes?;
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    let line = line.trim();
                    if !line.starts_with("data: ") { continue; }
                    let data = &line["data: ".len()..];
                    if data == "[DONE]" { break; }
                    let json: Value = serde_json::from_str(data)
                        .map_err(|e| HelloAgentException::llm(format!("SSE 解析错误: {}", e)))?;

                    if let Some(delta) = json["choices"].get(0).and_then(|c| c.get("delta")) {
                        if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                            yield content.to_string();
                        }
                        if let Some(reasoning) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
                            reasoning_buf.push_str(reasoning);
                        }
                    }
                    if let Some(u) = json.get("usage") {
                        usage_opt = serde_json::from_value(u.clone()).ok();
                    }
                }
            }

            let latency_ms = start.elapsed().as_millis() as i64;
            let usage = usage_opt.unwrap_or_default();
            *last_stats.lock().unwrap() = Some(StreamStats {
                model: model.clone(),
                usage,
                latency_ms,
                reasoning_content: if reasoning_buf.is_empty() { None } else { Some(reasoning_buf) },
            });
        };

        Box::pin(stream)
    }

    fn invoke_with_tools(
        &self,
        messages: Vec<Message>,
        tools: Vec<HashMap<String, Value>>,
        kwargs: HashMap<String, Value>,
    ) -> Result<LlmToolResponse, HelloAgentException> {
        let client = self
            .sync_client
            .clone()
            .map(Ok)
            .unwrap_or_else(|| self.create_sync_client())?;
        let start = Instant::now();

        let body = self.build_request_body(
            messages,
            kwargs,
            false,
            Some(tools),
            Some("auto".to_string()),
        );

        println!("==== {:?}", serde_json::to_string_pretty(&body));

        let url = format!("{}/chat/completions", self.base_url);
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(HelloAgentException::llm(format!("API 错误 ({}): {}", status, text)));
        }

        let latency_ms = start.elapsed().as_millis() as i64;
        let json: Value = resp.json()?;
        let choices = json["choices"]
            .as_array()
            .ok_or_else(|| HelloAgentException::llm("缺少 choices"))?;
        let choice = choices
            .first()
            .ok_or_else(|| HelloAgentException::llm("choices 为空"))?;
        let message = &choice["message"];
        let content = message["content"].as_str().map(|s| s.to_string());
        let model = json["model"].as_str().unwrap_or(&self.model).to_string();

        let tool_calls: Vec<ToolCall> =
            serde_json::from_value(message["tool_calls"].clone()).unwrap_or_default();
        let usage: Usage =
            serde_json::from_value(json["usage"].clone()).unwrap_or_default();

        Ok(LlmToolResponse::new(content, tool_calls, model, usage, latency_ms))
    }

    fn last_stats(&self) -> Option<StreamStats> {
        self.last_stats.lock().unwrap().clone()
    }
}

pub fn create_adapter(
    api_key: &str,
    base_url: &str,
    timeout: i32,
    model: &str,
) -> Result<Arc<dyn LLMAdapter>, HelloAgentException> {
    let adapter = OpenAIAdapter::new(api_key, base_url, timeout, model);
    Ok(Arc::new(adapter))           // Arc 支持从 Sized 类型创建
    // 或者使用 Arc::from(Box::new(adapter))，两者皆可。
}