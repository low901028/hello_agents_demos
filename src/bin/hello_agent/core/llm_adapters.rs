use crate::hello_agent::core::exceptions::HelloAgentsError;
use crate::hello_agent::core::llm_response::{LlmResponse, LlmToolResponse, ToolCall};
use async_trait::async_trait;
use futures::TryFutureExt;
use reqwest::Client;
use std::collections::HashMap;
use std::result;
use std::time::Instant;

#[async_trait]
pub trait BaseLlmAdapter: Send + Sync {
    fn api_key(&self) -> &str;
    fn base_url(&self) -> &str;
    fn timeout(&self) -> usize;
    fn model(&self) -> &str;
    fn invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, HelloAgentsError>;
    fn stream_invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<Box<dyn Iterator<Item = Result<String, HelloAgentsError>> + Send>, HelloAgentsError>;
    async fn astream_invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, HelloAgentsError>>, HelloAgentsError>;
    async fn ainvoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, HelloAgentsError>;
    fn invoke_with_tools(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        tools: &[HashMap<String, serde_json::Value>],
        tool_choice: &str,
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<LlmToolResponse, HelloAgentsError>;
    fn is_thinking_model(&self, m: &str) -> bool {
        ["reasoner", "o1", "o3", "thinking"]
            .iter()
            .any(|kw| m.to_lowercase().contains(kw))
    }
}

// ==================== OpenAI ====================

pub struct OpenAIAdapter {
    api_key: String,
    base_url: String,
    timeout: usize,
    model: String,
    client: Client,
}

impl OpenAIAdapter {
    pub fn new(
        api_key: &str,
        base_url: &str,
        timeout: usize,
        model: &str,
    ) -> Result<Self, HelloAgentsError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout as u64))
            .build()
            .map_err(|e| HelloAgentsError::llm(format!("创建HTTP客户端失败: {}", e)))?;
        Ok(OpenAIAdapter {
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            timeout,
            model: model.to_string(),
            client,
        })
    }

    fn build_body(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
        stream: bool,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({"model":self.model,"messages":messages,"temperature":temperature,"stream":stream});
        if let Some(mt) = max_tokens {
            body["max_tokens"] = serde_json::json!(mt);
        }
        body
    }

    fn extract_response(
        json: &serde_json::Value,
        model: &str,
        latency_ms: i64,
    ) -> Result<LlmResponse, HelloAgentsError> {
        let choices = json["choices"]
            .as_array()
            .ok_or_else(|| HelloAgentsError::llm("缺少choices"))?;
        let choice = choices
            .first()
            .ok_or_else(|| HelloAgentsError::llm("choices为空"))?;
        let content = choice["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let usage = {
            let u = &json["usage"];
            let mut map = HashMap::with_capacity(3);
            map.insert(
                "prompt_tokens".into(),
                u["prompt_tokens"].as_i64().unwrap_or(0),
            );
            map.insert(
                "completion_tokens".into(),
                u["completion_tokens"].as_i64().unwrap_or(0),
            );
            map.insert(
                "total_tokens".into(),
                u["total_tokens"].as_i64().unwrap_or(0),
            );
            map
        };
        let reasoning = choice["message"]["reasoning_content"]
            .as_str()
            .map(|s| s.to_string());
        Ok(LlmResponse {
            content,
            model: model.to_string(),
            usage,
            latency_ms,
            reasoning_content: reasoning,
        })
    }
}

#[async_trait]
impl BaseLlmAdapter for OpenAIAdapter {
    fn api_key(&self) -> &str {
        &self.api_key
    }
    fn base_url(&self) -> &str {
        &self.base_url
    }
    fn timeout(&self) -> usize {
        self.timeout
    }
    fn model(&self) -> &str {
        &self.model
    }

    fn invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, HelloAgentsError> {
        let start = Instant::now();
        let body = self.build_body(messages, temperature, max_tokens, false);
        let url = format!("{}/chat/completions", self.base_url);

        let response = tokio::task::block_in_place(|| {
            // 这里执行同步的 reqwest 请求
            reqwest::blocking::Client::new().post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
        });

        let resp = response.unwrap();

        if !resp.status().is_success() {
            return Err(HelloAgentsError::llm(format!(
                "API错误 {}: {}",
                resp.status(),
                resp.text().unwrap_or_default()
            )));
        }
        let json: serde_json::Value = resp
            .json()
            .map_err(|e| HelloAgentsError::llm(format!("JSON解析: {}", e)))?;
        Self::extract_response(&json, &self.model, start.elapsed().as_millis() as i64)
    }

    fn stream_invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<Box<dyn Iterator<Item = Result<String, HelloAgentsError>> + Send>, HelloAgentsError>
    {
        let body = self.build_body(messages, temperature, max_tokens, true);
        let url = format!("{}/chat/completions", self.base_url);

        let response = tokio::task::block_in_place(|| {
            // 这里执行同步的 reqwest 请求
            reqwest::blocking::Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .header("Accept", "text/event-stream")
                .json(&body)
                .send()
        });

        let resp = response.unwrap();
        let text = resp.text().unwrap_or_default();
        let chunks: Vec<Result<String, HelloAgentsError>> = text
            .lines()
            .filter(|l| l.starts_with("data: "))
            .filter_map(|l| {
                let s = l.strip_prefix("data: ").unwrap_or("");
                if s == "[DONE]" {
                    return None;
                }
                match serde_json::from_str::<serde_json::Value>(s) {
                    Ok(data) => {
                        let c = data["choices"][0]["delta"]["content"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        if c.is_empty() { None } else { Some(Ok(c)) }
                    }
                    Err(e) => Some(Err(HelloAgentsError::llm(format!("SSE:{}", e)))),
                }
            })
            .collect();
        Ok(Box::new(chunks.into_iter()))
    }

    async fn astream_invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, HelloAgentsError>>, HelloAgentsError>
    {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let body = self.build_body(messages, temperature, max_tokens, true);
        let url = format!("{}/chat/completions", self.base_url);
        let api_key = self.api_key.clone();
        tokio::spawn(async move {
            let client = Client::new();
            match client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    if let Ok(text) = resp.text().await {
                        for line in text.lines().filter(|l| l.starts_with("data: ")) {
                            let s = line.strip_prefix("data: ").unwrap_or("");
                            if s == "[DONE]" {
                                break;
                            }
                            if let Ok(data) = serde_json::from_str::<serde_json::Value>(s) {
                                if let Some(c) = data["choices"][0]["delta"]["content"].as_str() {
                                    if !c.is_empty() {
                                        let _ = tx.send(Ok(c.to_string())).await;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(Err(HelloAgentsError::llm(format!("流式失败:{}", e))))
                        .await;
                }
            }
        });
        Ok(rx)
    }

    async fn ainvoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, HelloAgentsError> {
        let start = Instant::now();
        let body = self.build_body(messages, temperature, max_tokens, false);
        let url = format!("{}/chat/completions", self.base_url);
        let client = Client::new();
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        let json: serde_json::Value = resp.json().await?;
        Self::extract_response(&json, &self.model, start.elapsed().as_millis() as i64)
    }

    fn invoke_with_tools(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        tools: &[HashMap<String, serde_json::Value>],
        tool_choice: &str,
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<LlmToolResponse, HelloAgentsError> {
        let start = Instant::now();
        let mut body = self.build_body(messages, temperature, max_tokens, false);
        body["tools"] = serde_json::json!(tools);
        body["tool_choice"] = serde_json::json!(tool_choice);
        let url = format!("{}/chat/completions", self.base_url);

        let response: serde_json::Value = tokio::task::block_in_place(|| {
            // 这里执行同步的 reqwest 请求
            reqwest::blocking::Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send().unwrap()
                .json().unwrap()
        });

        // let resp = response.unwrap();
        // let json: serde_json::Value = resp.json().unwrap();
        let json = response;
        let latency = start.elapsed().as_millis() as i64;
        let choice = json["choices"][0]
            .as_object()
            .ok_or_else(|| HelloAgentsError::llm("缺少choices"))?;
        let msg = &choice["message"];
        let content = msg["content"].as_str().map(|s| s.to_string());
        let tool_calls: Vec<ToolCall> = msg["tool_calls"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|tc| {
                        let f = &tc["function"];
                        Some(ToolCall {
                            id: tc["id"].as_str()?.to_string(),
                            name: f["name"].as_str()?.to_string(),
                            arguments: f["arguments"].as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let usage = {
            let u = &json["usage"];
            let mut map = HashMap::with_capacity(3);
            map.insert(
                "prompt_tokens".into(),
                u["prompt_tokens"].as_i64().unwrap_or(0),
            );
            map.insert(
                "completion_tokens".into(),
                u["completion_tokens"].as_i64().unwrap_or(0),
            );
            map.insert(
                "total_tokens".into(),
                u["total_tokens"].as_i64().unwrap_or(0),
            );
            map
        };
        Ok(LlmToolResponse {
            content,
            tool_calls,
            model: self.model.clone(),
            usage,
            latency_ms: latency,
        })
    }
}

// ==================== Anthropic ====================

pub struct AnthropicAdapter {
    api_key: String,
    base_url: String,
    timeout: usize,
    model: String,
    client: Client,
}

impl AnthropicAdapter {
    pub fn new(
        api_key: &str,
        base_url: &str,
        timeout: usize,
        model: &str,
    ) -> Result<Self, HelloAgentsError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout as u64))
            .build()
            .map_err(|e| HelloAgentsError::llm(format!("创建客户端失败: {}", e)))?;
        Ok(AnthropicAdapter {
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            timeout,
            model: model.to_string(),
            client,
        })
    }

    fn convert_messages(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
    ) -> (Option<String>, Vec<HashMap<String, serde_json::Value>>) {
        let mut system = None;
        let mut converted = Vec::with_capacity(messages.len());
        for msg in messages {
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            match role {
                "system" => {
                    system = msg
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
                "assistant" if msg.contains_key("tool_calls") => {
                    let mut blocks: Vec<serde_json::Value> = Vec::new();
                    if let Some(c) = msg.get("content").and_then(|v| v.as_str()) {
                        if !c.is_empty() {
                            blocks.push(serde_json::json!({"type":"text","text":c}));
                        }
                    }
                    if let Some(tcs) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                        for tc in tcs {
                            let f = tc.get("function").unwrap_or(&serde_json::Value::Null);
                            let args: serde_json::Value = f
                                .get("arguments")
                                .and_then(|v| v.as_str())
                                .and_then(|s| serde_json::from_str(s).ok())
                                .unwrap_or(serde_json::json!({}));
                            blocks.push(serde_json::json!({"type":"tool_use","id":tc.get("id"),"name":f.get("name"),"input":args}));
                        }
                    }
                    let mut m = HashMap::new();
                    m.insert("role".into(), serde_json::json!("assistant"));
                    m.insert("content".into(), serde_json::json!(blocks));
                    converted.push(m);
                }
                "tool" => {
                    let mut m = HashMap::new();
                    m.insert("role".into(), serde_json::json!("user"));
                    m.insert("content".into(), serde_json::json!([{"type":"tool_result","tool_use_id":msg.get("tool_call_id"),"content":msg.get("content")}]));
                    converted.push(m);
                }
                _ => {
                    converted.push(msg.clone());
                }
            }
        }
        (system, converted)
    }
}

#[async_trait]
impl BaseLlmAdapter for AnthropicAdapter {
    fn api_key(&self) -> &str {
        &self.api_key
    }
    fn base_url(&self) -> &str {
        &self.base_url
    }
    fn timeout(&self) -> usize {
        self.timeout
    }
    fn model(&self) -> &str {
        &self.model
    }

    fn invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, HelloAgentsError> {
        let start = Instant::now();
        let (system, converted) = self.convert_messages(messages);
        let mut body = serde_json::json!({"model":self.model,"messages":converted,"max_tokens":max_tokens.unwrap_or(4096),"temperature":temperature});
        if let Some(sys) = &system {
            body["system"] = serde_json::json!(sys);
        }
        let url = format!("{}/messages", self.base_url);

        let response = tokio::task::block_in_place(|| {
            // 这里执行同步的 reqwest 请求
            reqwest::blocking::Client::new().post(&url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
        });

        let resp = response.unwrap();
        let json: serde_json::Value = resp.json()?;
        let latency = start.elapsed().as_millis() as i64;
        let mut content = String::new();
        if let Some(blocks) = json["content"].as_array() {
            for b in blocks {
                if let Some(t) = b.get("text").and_then(|v| v.as_str()) {
                    content.push_str(t);
                }
            }
        }
        let usage = {
            let u = &json["usage"];
            let mut map = HashMap::with_capacity(3);
            let i = u["input_tokens"].as_i64().unwrap_or(0);
            let o = u["output_tokens"].as_i64().unwrap_or(0);
            map.insert("prompt_tokens".into(), i);
            map.insert("completion_tokens".into(), o);
            map.insert("total_tokens".into(), i + o);
            map
        };
        Ok(LlmResponse {
            content,
            model: self.model.clone(),
            usage,
            latency_ms: latency,
            reasoning_content: None,
        })
    }

    fn stream_invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<Box<dyn Iterator<Item = Result<String, HelloAgentsError>> + Send>, HelloAgentsError>
    {
        let (system, converted) = self.convert_messages(messages);
        let mut body = serde_json::json!({"model":self.model,"messages":converted,"max_tokens":max_tokens.unwrap_or(4096),"temperature":temperature,"stream":true});
        if let Some(sys) = &system {
            body["system"] = serde_json::json!(sys);
        }
        let url = format!("{}/messages", self.base_url);
        let response = tokio::task::block_in_place(|| {
            // 这里执行同步的 reqwest 请求
            reqwest::blocking::Client::new()
                .post(&url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
        });

        let resp = response.unwrap();
        let text = resp.text().unwrap_or_default();
        let chunks: Vec<Result<String, HelloAgentsError>> = text
            .lines()
            .filter(|l| l.starts_with("data: "))
            .filter_map(|l| {
                let s = l.strip_prefix("data: ").unwrap_or("");
                match serde_json::from_str::<serde_json::Value>(s) {
                    Ok(data) => {
                        if let Some(d) = data.get("delta") {
                            d.get("text")
                                .and_then(|v| v.as_str())
                                .map(|t| Ok(t.to_string()))
                        } else if data.get("type").and_then(|v| v.as_str())
                            == Some("content_block_delta")
                        {
                            data.get("delta")
                                .and_then(|d| d.get("text"))
                                .and_then(|v| v.as_str())
                                .map(|t| Ok(t.to_string()))
                        } else {
                            None
                        }
                    }
                    Err(e) => Some(Err(HelloAgentsError::llm(format!("SSE:{}", e)))),
                }
            })
            .collect();
        Ok(Box::new(chunks.into_iter()))
    }

    async fn astream_invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, HelloAgentsError>>, HelloAgentsError>
    {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let (system, converted) = self.convert_messages(messages);
        let mut body = serde_json::json!({"model":self.model,"messages":converted,"max_tokens":max_tokens.unwrap_or(4096),"temperature":temperature,"stream":true});
        if let Some(sys) = &system {
            body["system"] = serde_json::json!(sys);
        }
        let url = format!("{}/messages", self.base_url);
        let api_key = self.api_key.clone();
        tokio::spawn(async move {
            let client = Client::new();
            match client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    if let Ok(text) = resp.text().await {
                        for line in text.lines().filter(|l| l.starts_with("data: ")) {
                            if let Ok(data) = serde_json::from_str::<serde_json::Value>(
                                line.strip_prefix("data: ").unwrap_or(""),
                            ) {
                                if let Some(t) = data
                                    .get("delta")
                                    .and_then(|d| d.get("text"))
                                    .and_then(|v| v.as_str())
                                {
                                    let _ = tx.send(Ok(t.to_string())).await;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(Err(HelloAgentsError::llm(format!("流式失败:{}", e))))
                        .await;
                }
            }
        });
        Ok(rx)
    }

    async fn ainvoke(
        &self,
        _messages: &[HashMap<String, serde_json::Value>],
        _temperature: f64,
        _max_tokens: Option<usize>,
    ) -> Result<LlmResponse, HelloAgentsError> {
        Err(HelloAgentsError::llm("Anthropic异步invoke暂未实现"))
    }

    fn invoke_with_tools(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        tools: &[HashMap<String, serde_json::Value>],
        _tool_choice: &str,
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<LlmToolResponse, HelloAgentsError> {
        let start = Instant::now();
        let (system, converted) = self.convert_messages(messages);
        let mut body = serde_json::json!({"model":self.model,"messages":converted,"tools":tools,"max_tokens":max_tokens.unwrap_or(4096),"temperature":temperature});
        if let Some(sys) = &system {
            body["system"] = serde_json::json!(sys);
        }
        let url = format!("{}/messages", self.base_url);

        let response = tokio::task::block_in_place(|| {
            // 这里执行同步的 reqwest 请求
            reqwest::blocking::Client::new()
                .post(&url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
        });

        let resp = response.unwrap();
        let json: serde_json::Value = resp.json()?;
        let latency = start.elapsed().as_millis() as i64;
        let mut content = String::new();
        let mut tool_calls = Vec::new();
        if let Some(blocks) = json["content"].as_array() {
            for b in blocks {
                match b["type"].as_str() {
                    Some("text") => {
                        if let Some(t) = b["text"].as_str() {
                            content.push_str(t);
                        }
                    }
                    Some("tool_use") => {
                        tool_calls.push(ToolCall {
                            id: b["id"].as_str().unwrap_or("").to_string(),
                            name: b["name"].as_str().unwrap_or("").to_string(),
                            arguments: serde_json::to_string(&b["input"]).unwrap_or_default(),
                        });
                    }
                    _ => {}
                }
            }
        }
        let usage = {
            let u = &json["usage"];
            let mut map = HashMap::with_capacity(3);
            let i = u["input_tokens"].as_i64().unwrap_or(0);
            let o = u["output_tokens"].as_i64().unwrap_or(0);
            map.insert("prompt_tokens".into(), i);
            map.insert("completion_tokens".into(), o);
            map.insert("total_tokens".into(), i + o);
            map
        };
        Ok(LlmToolResponse {
            content: if content.is_empty() {
                None
            } else {
                Some(content)
            },
            tool_calls,
            model: self.model.clone(),
            usage,
            latency_ms: latency,
        })
    }
}

// ==================== Gemini ====================

pub struct GeminiAdapter {
    api_key: String,
    base_url: String,
    timeout: usize,
    model: String,
    client: Client,
}

impl GeminiAdapter {
    pub fn new(
        api_key: &str,
        base_url: &str,
        timeout: usize,
        model: &str,
    ) -> Result<Self, HelloAgentsError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout as u64))
            .build()
            .map_err(|e| HelloAgentsError::llm(format!("创建客户端失败: {}", e)))?;
        Ok(GeminiAdapter {
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            timeout,
            model: model.to_string(),
            client,
        })
    }
}

#[async_trait]
impl BaseLlmAdapter for GeminiAdapter {
    fn api_key(&self) -> &str {
        &self.api_key
    }
    fn base_url(&self) -> &str {
        &self.base_url
    }
    fn timeout(&self) -> usize {
        self.timeout
    }
    fn model(&self) -> &str {
        &self.model
    }

    fn invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, HelloAgentsError> {
        let start = Instant::now();
        let mut body =
            serde_json::json!({"contents":messages,"generationConfig":{"temperature":temperature}});
        if let Some(mt) = max_tokens {
            body["generationConfig"]["maxOutputTokens"] = serde_json::json!(mt);
        }
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.base_url, self.model, self.api_key
        );

        let response = tokio::task::block_in_place(|| {
            // 这里执行同步的 reqwest 请求
            reqwest::blocking::Client::new()
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
        });

        let resp = response.unwrap();
        let json: serde_json::Value = resp.json()?;
        let latency = start.elapsed().as_millis() as i64;
        let content = json["candidates"][0]["content"]["parts"]
            .as_array()
            .map(|p| {
                p.iter()
                    .filter_map(|pp| pp["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();
        let usage = {
            let u = &json["usageMetadata"];
            let mut map = HashMap::with_capacity(3);
            map.insert(
                "prompt_tokens".into(),
                u["promptTokenCount"].as_i64().unwrap_or(0),
            );
            map.insert(
                "completion_tokens".into(),
                u["candidatesTokenCount"].as_i64().unwrap_or(0),
            );
            map.insert(
                "total_tokens".into(),
                u["totalTokenCount"].as_i64().unwrap_or(0),
            );
            map
        };
        Ok(LlmResponse {
            content,
            model: self.model.clone(),
            usage,
            latency_ms: latency,
            reasoning_content: None,
        })
    }

    fn stream_invoke(
        &self,
        messages: &[HashMap<String, serde_json::Value>],
        temperature: f64,
        max_tokens: Option<usize>,
    ) -> Result<Box<dyn Iterator<Item = Result<String, HelloAgentsError>> + Send>, HelloAgentsError>
    {
        let mut body =
            serde_json::json!({"contents":messages,"generationConfig":{"temperature":temperature}});
        if let Some(mt) = max_tokens {
            body["generationConfig"]["maxOutputTokens"] = serde_json::json!(mt);
        }
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.base_url, self.model, self.api_key
        );
        let response = tokio::task::block_in_place(|| {
            // 这里执行同步的 reqwest 请求
            reqwest::blocking::Client::new()
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
        });

        let resp = response.unwrap();
        let text = resp.text().unwrap_or_default();
        let chunks: Vec<Result<String, HelloAgentsError>> = text
            .lines()
            .filter(|l| l.starts_with("data: "))
            .filter_map(|l| {
                match serde_json::from_str::<serde_json::Value>(
                    l.strip_prefix("data: ").unwrap_or(""),
                ) {
                    Ok(data) => {
                        let c = data["candidates"][0]["content"]["parts"]
                            .as_array()
                            .map(|p| {
                                p.iter()
                                    .filter_map(|pp| pp["text"].as_str())
                                    .collect::<Vec<_>>()
                                    .join("")
                            })
                            .unwrap_or_default();
                        if c.is_empty() { None } else { Some(Ok(c)) }
                    }
                    Err(e) => Some(Err(HelloAgentsError::llm(format!("SSE:{}", e)))),
                }
            })
            .collect();
        Ok(Box::new(chunks.into_iter()))
    }

    async fn astream_invoke(
        &self,
        _messages: &[HashMap<String, serde_json::Value>],
        _temperature: f64,
        _max_tokens: Option<usize>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, HelloAgentsError>>, HelloAgentsError>
    {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let _ = tx
            .send(Err(HelloAgentsError::llm("Gemini异步流式暂未实现")))
            .await;
        Ok(rx)
    }

    async fn ainvoke(
        &self,
        _messages: &[HashMap<String, serde_json::Value>],
        _temperature: f64,
        _max_tokens: Option<usize>,
    ) -> Result<LlmResponse, HelloAgentsError> {
        Err(HelloAgentsError::llm("Gemini异步invoke暂未实现"))
    }

    fn invoke_with_tools(
        &self,
        _messages: &[HashMap<String, serde_json::Value>],
        _tools: &[HashMap<String, serde_json::Value>],
        _tool_choice: &str,
        _temperature: f64,
        _max_tokens: Option<usize>,
    ) -> Result<LlmToolResponse, HelloAgentsError> {
        Err(HelloAgentsError::llm("Gemini工具调用暂未实现"))
    }
}

// ==================== 工厂函数 ====================

pub fn create_adapter(
    api_key: &str,
    base_url: &str,
    timeout: usize,
    model: &str,
) -> Result<Box<dyn BaseLlmAdapter>, HelloAgentsError> {
    let lower = base_url.to_lowercase();
    if lower.contains("anthropic.com") {
        Ok(Box::new(AnthropicAdapter::new(
            api_key, base_url, timeout, model,
        )?))
    } else if lower.contains("googleapis.com") || lower.contains("generativelanguage") {
        Ok(Box::new(GeminiAdapter::new(
            api_key, base_url, timeout, model,
        )?))
    } else {
        Ok(Box::new(OpenAIAdapter::new(
            api_key, base_url, timeout, model,
        )?))
    }
}
