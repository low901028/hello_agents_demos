use std::env;
use std::sync::Arc;
use std::io::{self, Write};
use std::time::Duration;
use futures::StreamExt;
use reqwest::Client;
use simple_hello_agents_base::llm_client::{LLMClient};
use simple_hello_agents_base::client_message::{ChatChunk, ChatRequest, ChatResponse, Message};
use crate::Config::Config;

const DEFAULT_MAX_TOKENS: usize = 128_000;
const DEFAULT_MAX_TIMEOUT: Duration = Duration::from_secs(60);

/// =========================
/// 模型供应商
/// =========================
pub struct Provider {
    // pub model: String,
    pub api_key: String,
    pub base_url: String,
    // pub provider: String,
    // pub temperature: f32,
    // pub max_tokens: usize,
    pub config: Config,
    pub timeout: Duration,
    pub llm_client: Arc<Client>,
}

impl Provider {
    pub fn new(model: Option<String>,     // 模型
           api_key: Option<String>,   // api key
           base_url: Option<String>,  // base url
           provider: Option<String>,  // 模型供应商
           temperature: Option<f32>,  // 热度
           max_tokens: Option<usize>, // 当前模型最大的上下文
           timeout: Option<Duration>, // http timeout
           llm_client: Option<Arc<Client>>, // http client
    ) -> Self { // http timeout
        dotenvy::dotenv().ok();

        let model = model
            .or_else(|| env::var("LLM_MODEL_ID").ok())
            .unwrap_or_else(|| "deepseek-chat".to_string());
        let api_key = api_key
            .or_else(|| env::var("LLM_API_KEY").ok())
            .expect("API key must be provided or set in LLM_API_KEY env var");
        let base_url = base_url
            .or_else(|| env::var("LLM_BASE_URL").ok())
            .unwrap_or_else(|| "https://api.deepseek.com".to_string());
        let provider = provider.or_else(|| env::var("LLM_MODEL_PROVIDER").ok())
            .unwrap_or_else(|| "deepseek".to_string());
        let temperature = temperature.unwrap_or(0.7);
        let max_tokens = max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
        let timeout = timeout.or_else(||{
            let timeout = env::var("LLM_BASE_TIMEOUT").ok().unwrap();
            let timeout = timeout.parse().unwrap();
            Some(Duration::from_secs(timeout))
        }).unwrap_or_else(|| DEFAULT_MAX_TIMEOUT);

        let http_client = llm_client.unwrap_or_else(||{
            Arc::new(reqwest::ClientBuilder::new()
                .timeout(timeout)
                .build()
                .unwrap())
        });

        let config = Config::new(Some(model),
                                 Some(provider),
                                 Some(temperature),
                                 Some(max_tokens),
                                 None,
                                 None,
                                 Some(200));

        Provider {
            // model,
            api_key,
            base_url,
            // provider,
            // temperature,
            // max_tokens,
            config,
            timeout,
            llm_client: http_client,
        }

    }
}

/// ============================================= ///
/// 扩展原有的LLM Client
/// ============================================= ///
pub struct MyLLM {
    pub llm_client: Option<Arc<LLMClient>>,
    pub provider: Provider,
}

impl MyLLM {
    pub fn new(llm_client: Option<Arc<LLMClient>>,
               provider: Provider) -> Self {
        MyLLM {
            llm_client,
            provider,
        }
    }

    /// 自动检测LLM提供商
    ///
    ///         检测逻辑：
    ///         1. 优先检查特定提供商的环境变量
    ///         2. 根据API密钥格式判断
    ///         3. 根据base_url判断
    ///         4. 默认返回通用配置
    pub fn auto_detect_provider(&self) -> Result<String, Box<dyn std::error::Error>> {
        let provider = &self.provider.config.provider;
        if !provider.is_empty() {
            return Ok(provider.to_string());
        }

        //
        let api_key = &self.provider.api_key;
        let api_key_lower = api_key.to_ascii_lowercase();
        let provider = match api_key_lower.as_str() {
            "ollama" => "ollama",
            "vllm" => "vllm",
            "local" => "local",
            (s) => {
                if s.starts_with("ms-") {
                    "modelscope"
                } else if s.starts_with("sk-") && s.len() > 50 { // deepseek/kimi/openai均有可能 需分类处理
                    ""
                } else if s.starts_with(".") || s[(s.len()-20)..].contains('.') {
                    "zhipu"
                } else {
                    ""
                }
            }
            _ => ""
        };
        if !provider.is_empty() {
            return Ok(provider.to_string());
        }

        let base_url = &self.provider.base_url;
        if base_url.contains("api.deepseek.com") {
            return Ok("deepseek".to_string());
        } else if base_url.contains('/') {}

        //
        return Ok("auto".to_ascii_lowercase())
    }

    /// 发送对话请求，返回完整响应文本（自动判断流/非流）
    pub async fn chat(
        &self,
        messages: Vec<Message>,
        temperature: f32,
        stream: bool,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!("{}/chat/completions", self.provider.base_url);

        let request_body = ChatRequest {
            model: self.provider.config.model.to_string(),
            messages,
            temperature,
            stream,
        };

        let resp = self.provider.llm_client.clone()
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.provider.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await?;
            return Err(format!("API 错误 ({}): {}", status, text).into());
        }

        if stream {
            self.handle_stream(resp).await
        } else {
            self.handle_no_stream(resp).await
        }
    }

    /// 处理流式响应（SSE）
    async fn handle_stream(
        &self,
        resp: reqwest::Response,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut stream = resp.bytes_stream();
        let mut collected = String::new();
        println!("🤖 流式响应：\n");

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
                if let Some(data) = line.strip_prefix("test_data: ") {
                    if data == "[DONE]" {
                        break;
                    }
                    match serde_json::from_str::<ChatChunk>(data) {
                        Ok(chunk_data) => {
                            if let Some(choice) = chunk_data.choices.first() {
                                if let Some(content) = &choice.delta.content {
                                    print!("{}", content);
                                    io::stdout().flush().unwrap();
                                    collected.push_str(content);
                                }
                            }
                        }
                        Err(e) => eprintln!("\n⚠️ 解析流块失败: {}", e),
                    }
                }
            }
        }
        println!();
        Ok(collected)
    }

    /// 处理非流式响应
    async fn handle_no_stream(
        &self,
        resp: reqwest::Response,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let chat_resp: ChatResponse = resp.json().await?;
        let content = chat_resp
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();
        println!("🤖 完整响应：\n{}", content);
        Ok(content)
    }

    pub async fn think(
        &self,
        stream: bool,
        messages: Vec<Message>,
        temperature: f32)-> Result<String, Box<dyn std::error::Error>> {
        let llm_client = self.llm_client.clone().unwrap();
        let llm_client = llm_client.clone();
        match self.provider.config.provider.to_ascii_lowercase().as_str() {
            "modelscope" => {
                self.chat(messages, temperature, stream).await
            },
            _ => { // 默认走deepseek
                llm_client.think(messages, temperature).await
            }
        }
    }
}



