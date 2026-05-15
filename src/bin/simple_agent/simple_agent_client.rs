use anyhow::{Context, Result};
use dotenvy::dotenv;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

/// 与 OpenAI 兼容的消息结构
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub (crate) struct Message {
    pub role: String,
    pub content: String,
    #[serde(default,skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,   // 新增字段，兼容 OpenAI 格式
}

/// 请求体
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f64,
    stream: bool,
}

/// 流式响应的单个 chunk（只关心 delta）
#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Delta,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

/// HelloAgentsLLM 客户端
pub struct HelloAgentsLLM {
    model: String,
    api_key: String,
    base_url: String,
    client: Client,
}

impl HelloAgentsLLM {
    /// 创建客户端
    /// - `model`: 模型名称，None 则从环境变量 `LLM_MODEL_ID` 读取
    /// - `api_key`: API 密钥，None 则从环境变量 `LLM_API_KEY` 读取
    /// - `base_url`: 服务地址，None 则从环境变量 `LLM_BASE_URL` 读取
    /// - `timeout_secs`: 超时秒数，None 则从环境变量 `LLM_TIMEOUT` 读取（默认 60）
    pub fn new(
        model: Option<&str>,
        api_key: Option<&str>,
        base_url: Option<&str>,
        timeout_secs: Option<u64>,
    ) -> Result<Self> {
        let model = model
            .map(|s| s.to_string())
            .context("模型 ID 未提供")?;

        let api_key = api_key
            .map(|s| s.to_string())
            .context("API 密钥未提供")?;

        let base_url = base_url
            .map(|s| s.to_string())
            .context("服务地址未提供")?;

        let timeout = timeout_secs
            .unwrap_or(60);

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout))
            .build()?;

        Ok(Self {
            model,
            api_key,
            base_url,
            client,
        })
    }

    /// 调用 LLM 思考，返回完整响应文本
    pub async fn think(&self, messages: Vec<Message>, temperature: f64) -> Result<String> {
        println!("🧠 正在调用 {} 模型...", self.model);

        let body = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            temperature,
            stream: true,
        };

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("LLM API 请求失败")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("请求失败: {} - {}", status, text));
        }

        println!("✅ 大语言模型响应成功:");

        let mut collected = String::new();
        let mut stream = resp.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("读取流数据失败")?;
            let text = String::from_utf8_lossy(&chunk);

            // SSE 格式: "data: ...\n\n"
            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                        if let Some(first_choice) = chunk.choices.first() {
                            if let Some(content) = &first_choice.delta.content {
                                print!("{}", content);
                                use std::io::Write;
                                std::io::stdout().flush().ok();
                                collected.push_str(content);
                            }
                        }
                    }
                }
            }
        }

        println!(); // 流式输出结束后换行
        Ok(collected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // 辅助函数：创建测试用的消息
    fn test_messages() -> Vec<Message> {
        vec![
            Message {
                role: "system".into(),
                content: "You are a test assistant.".into(),
                ..Default::default()
            },
            Message {
                role: "user".into(),
                content: "Hello".into(),
                ..Default::default()
            },
        ]
    }

    // 测试 1: 构造客户端成功（使用模拟值）
    #[test]
    fn test_client_creation_success() {
        // 设置临时环境变量（避免影响全局）
        unsafe {
            std::env::set_var("LLM_MODEL_ID", "test-model");
            std::env::set_var("LLM_API_KEY", "test-key");
            std::env::set_var("LLM_BASE_URL", "http://localhost:1234");
        }

        let client = HelloAgentsLLM::new(None, None, None, None);
        assert!(client.is_ok());
    }

    // 测试 2: 构造客户端失败（缺少参数）
    #[test]
    fn test_client_creation_missing_params() {
        // 清除环境变量并确保 .env 不存在（或提供新环境）
        // 这里直接传入部分参数
        let client = HelloAgentsLLM::new(Some("model"), Some("key"), None, None);
        assert!(client.is_err());
        let err = client.err().unwrap().to_string();
        assert!(err.contains("服务地址") || err.contains("LLM_BASE_URL"));
    }

    // 测试 3: 流式响应正常处理
    #[tokio::test]
    async fn test_think_stream_success() {
        let mock_server = MockServer::start().await;

        // 模拟返回 SSE 流
        let response_body = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n\n\
                             data: {\"choices\":[{\"delta\":{\"content\":\"World\"}}]}\n\n\
                             data: [DONE]\n\n";

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("Authorization", "Bearer test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(response_body)
                    .insert_header("Content-Type", "text/event-stream"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = HelloAgentsLLM::new(
            Some("test-model"),
            Some("test-key"),
            Some(&mock_server.uri()),
            Some(10),
        )
            .unwrap();

        let messages = test_messages();
        let result = client.think(messages, 0.0).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello World");
    }

    // 测试 4: HTTP 错误
    #[tokio::test]
    async fn test_think_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Error"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = HelloAgentsLLM::new(
            Some("test-model"),
            Some("test-key"),
            Some(&mock_server.uri()),
            Some(10),
        )
            .unwrap();

        let result = client.think(test_messages(), 0.0).await;
        assert!(result.is_err());
        let err_str = result.err().unwrap().to_string();
        assert!(err_str.contains("500") || err_str.contains("Internal Error"));
    }

    // 测试 5: 无有效数据的流
    #[tokio::test]
    async fn test_think_empty_stream() {
        let mock_server = MockServer::start().await;

        // 只有 [DONE] 无内容
        let response_body = "data: [DONE]\n\n";

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(response_body)
                    .insert_header("Content-Type", "text/event-stream"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = HelloAgentsLLM::new(
            Some("test-model"),
            Some("test-key"),
            Some(&mock_server.uri()),
            Some(10),
        )
            .unwrap();

        let result = client.think(test_messages(), 0.0).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ""); // 空字符串
    }
}

fn main() {}