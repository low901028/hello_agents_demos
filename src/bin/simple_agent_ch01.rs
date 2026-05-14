use anyhow::Context;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use dotenvy::dotenv;

// ==================== 系统提示 ====================
const AGENT_SYSTEM_PROMPT: &str = r#"你是一个智能旅行助手。你的任务是分析用户的请求，并使用可用工具一步步地解决问题。

# 可用工具:
- `get_weather(city: str)`: 查询指定城市的实时天气。
- `get_attraction(city: str, weather: str)`: 根据城市和天气搜索推荐的旅游景点。

# 输出格式要求:
你的每次回复必须严格遵循以下格式，包含一对Thought和Action：

Thought: [你的思考过程和下一步计划]
Action: [你要执行的具体行动]

Action的格式必须是以下之一：
1. 调用工具：function_name(arg_name="arg_value")
2. 结束任务：Finish[最终答案]

# 重要提示:
- 每次只输出一对Thought-Action
- Action必须在同一行，不要换行
- 当收集到足够信息可以回答用户问题时，必须使用 Action: Finish[最终答案] 格式结束

请开始吧！
"#;

// ==================== 工具抽象 ====================

#[async_trait]
pub trait ToolExecutor {
    async fn get_weather(&self, city: &str) -> Result<String, String>;
    async fn get_attraction(&self, city: &str, weather: &str) -> Result<String, String>;
}

/// 基于真实 API 的工具实现
pub struct RealToolExecutor {
    tavily_api_key: String,
}

impl RealToolExecutor {
    pub fn new(tavily_api_key: String) -> Self {
        Self { tavily_api_key }
    }
}

#[async_trait]
impl ToolExecutor for RealToolExecutor {
    async fn get_weather(&self, city: &str) -> Result<String, String> {
        let url = format!("https://wttr.in/{}?format=j1", city);
        let response = reqwest::get(&url)
            .await
            .map_err(|e| format!("网络错误: {}", e))?;
        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("解析JSON失败: {}", e))?;

        let current = data
            .get("current_condition")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| "无法获取天气数据".to_string())?;

        let desc = current
            .get("weatherDesc")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("未知");

        let temp = current
            .get("temp_C")
            .and_then(|v| v.as_str())
            .unwrap_or("?");

        Ok(format!("{}当前天气：{}，气温{}℃", city, desc, temp))
    }

    async fn get_attraction(&self, city: &str, weather: &str) -> Result<String, String> {
        if self.tavily_api_key.is_empty() {
            return Err("未配置 TAVILY_API_KEY".to_string());
        }

        let client = reqwest::Client::new();
        let query = format!("'{}' 在'{}'天气下最值得去的旅游景点推荐及理由", city, weather);
        let body = serde_json::json!({
            "api_key": self.tavily_api_key,
            "query": query,
            "search_depth": "basic",
            "include_answer": true
        });

        let resp = client
            .post("https://api.tavily.com/search")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Tavily 请求失败: {}", e))?;

        let data: serde_json::Value = resp.json().await.map_err(|e| format!("解析失败: {}", e))?;

        if let Some(answer) = data["answer"].as_str() {
            if !answer.is_empty() {
                return Ok(answer.to_string());
            }
        }

        let results = data["results"]
            .as_array()
            .ok_or("未找到景点信息")?;
        let items: Vec<String> = results
            .iter()
            .filter_map(|r| {
                let title = r["title"].as_str().unwrap_or("");
                let content = r["content"].as_str().unwrap_or("");
                if title.is_empty() && content.is_empty() {
                    None
                } else {
                    Some(format!("- {}: {}", title, content))
                }
            })
            .collect();

        if items.is_empty() {
            Err("未找到相关景点推荐".to_string())
        } else {
            Ok(format!("根据搜索，为您找到以下信息：\n{}", items.join("\n")))
        }
    }
}

// ==================== DeepSeek 客户端（基于 V4 API 更新） ====================

struct DeepSeekClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl DeepSeekClient {
    fn new(api_key: String, base_url: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url,
            model,
        }
    }

    async fn chat(&self, messages: Vec<Message>) -> Result<String, String> {
        let body = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature: 1.0,                  // 官方推荐
            top_p: Some(1.0),                  // 官方推荐
            max_tokens: Some(2048),            // 合理限制输出
            thinking: Some(ThinkingConfig {
                r#type: "disabled".to_string(),  // Agent 工具调用不需要思考链
            }),
            stream: false,
        };

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        let data: ChatResponse = resp.json().await.map_err(|e| format!("解析失败: {}", e))?;

        data.choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| "响应中没有 choices".to_string())
    }
}

// ==================== 数据结构（基于 V4 API） ====================

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
    stream: bool,
}

#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    r#type: String, // "enabled" 或 "disabled"
}

#[derive(Serialize, Deserialize, Clone)]
struct Message {
    role: String, // "system", "user", "assistant"
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

// ==================== 解析工具 ====================

fn parse_action(action: &str) -> Option<(String, HashMap<String, String>)> {
    let re = Regex::new(r#"(\w+)\((.*)\)"#).unwrap();
    let caps = re.captures(action)?;
    let tool_name = caps.get(1)?.as_str().to_string();
    let args_str = caps.get(2)?.as_str();

    let arg_re = Regex::new(r#"(\w+)=\\"([^\\"]*)\\"#).unwrap();
    let mut kwargs = HashMap::new();
    for cap in arg_re.captures_iter(args_str) {
        kwargs.insert(cap[1].to_string(), cap[2].to_string());
    }
    Some((tool_name, kwargs))
}

fn truncate_thought_action(text: &str) -> String {
    // let re = Regex::new(
    //     r"(?s)(Thought:.*?Action:.*?)(?=\n\s*(?:Thought:|Action:|Observation:)|\z)",
    // )
    // (?s) 使 . 匹配换行，Thought:.*? 非贪婪匹配到第一个 Action:
    // Action:[^\n]* 匹配 Action 行直到行尾（无内部换行）
    let re = Regex::new(r"(?s)(Thought:.*?Action:.*?)(?:\n\s*(?:Thought:|Action:|Observation:)|\z)").unwrap();
    if let Some(m) = re.find(text) {
        let trimmed = m.as_str().trim().to_string();
        if trimmed != text.trim() {
            println!("⚠️ 已截断多余的 Thought-Action 对");
            trimmed
        } else {
            text.to_string()
        }
    } else {
        text.to_string()
    }
}

// ==================== 主循环 ====================

async fn run_agent(
    client: &DeepSeekClient,
    tools: &dyn ToolExecutor,
    user_request: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut history = vec![format!("用户请求: {}", user_request)];
    println!("用户: {}\n{}", user_request, "=".repeat(40));

    for turn in 1..=5 {
        println!("--- 第 {} 轮 ---\n", turn);

        let prompt = history.join("\n");
        let messages = vec![
            Message {
                role: "system".into(),
                content: AGENT_SYSTEM_PROMPT.into(),
            },
            Message {
                role: "user".into(),
                content: prompt,
            },
        ];

        let llm_output = match client.chat(messages).await {
            Ok(o) => o,
            Err(e) => {
                let obs = format!("错误：调用语言模型失败 - {}", e);
                history.push(format!("Observation: {}", obs));
                continue;
            }
        };

        println!("======原始达模型输出：\n{}\n", &llm_output);
        let llm_output = truncate_thought_action(&llm_output);
        println!("模型输出:\n{}\n", llm_output);
        history.push(llm_output.clone());

        let action_re = Regex::new(r"Action:\s*(.*)").unwrap();
        let action_str = match action_re.captures(&llm_output) {
            Some(c) => c.get(1).unwrap().as_str().trim().to_string(),
            None => {
                let obs = "错误: 未能解析到 Action，请遵循 Thought-Action 格式";
                history.push(format!("Observation: {}", obs));
                continue;
            }
        };

        if let Some(caps) = Regex::new(r"Finish\[(.*)\]").unwrap().captures(&action_str) {
            let answer = caps.get(1).unwrap().as_str().to_string();
            println!("✅ 任务完成");
            return Ok(answer);
        }

        let (tool_name, args) = match parse_action(&action_str) {
            Some(p) => p,
            None => {
                let obs = format!("错误：无法解析 Action: '{}'", action_str);
                history.push(format!("Observation: {}", obs));
                continue;
            }
        };

        let observation = match tool_name.as_str() {
            "get_weather" => {
                let city = args.get("city").cloned().unwrap_or_default();
                tools
                    .get_weather(&city)
                    .await
                    .unwrap_or_else(|e| format!("天气查询失败: {}", e))
            }
            "get_attraction" => {
                let city = args.get("city").cloned().unwrap_or_default();
                let weather = args.get("weather").cloned().unwrap_or_default();
                tools
                    .get_attraction(&city, &weather)
                    .await
                    .unwrap_or_else(|e| format!("景点查询失败: {}", e))
            }
            _ => format!("错误：未定义的工具 '{}'", tool_name),
        };

        let observation_line = format!("Observation: {}", observation);
        println!("{}\n{}", observation_line, "=".repeat(40));
        history.push(observation_line);
    }

    Err("超过最大循环次数，未能完成用户请求".into())
}

// ==================== 程序入口 ====================
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let deepseek_key = env::var("DEEPSEEK_API_KEY").context("请设置 DEEPSEEK_API_KEY")?;
    let tavily_key = env::var("TAVILY_API_KEY").unwrap_or_default();
    // let deepseek_key = "sk-36b810755bd84b78a003cf586dba9bef".to_string();
    // let tavily_key = "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".to_string();

    // 使用最新 V4 模型（deepseek-v4-pro）
    let client = DeepSeekClient::new(
        deepseek_key,
        "https://api.deepseek.com".to_string(),
        "deepseek-v4-flash".to_string(), // 或 "deepseek-v4-flash"
    );

    let tools = RealToolExecutor::new(tavily_key);

    let request = "你好，请帮我查询一下今天北京的天气，然后根据天气推荐一个合适的旅游景点。";
    let answer = run_agent(&client, &tools, request).await?;
    println!("\n🎉 最终答案:\n{}", answer);
    Ok(())
}

// ==================== 测试（保持不变） ====================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct MockToolExecutor {
        weather_response: Mutex<Option<String>>,
        attraction_response: Mutex<Option<String>>,
    }

    impl MockToolExecutor {
        fn new() -> Self {
            Self {
                weather_response: Mutex::new(None),
                attraction_response: Mutex::new(None),
            }
        }

        fn set_weather(&self, response: String) {
            *self.weather_response.lock().unwrap() = Some(response);
        }

        fn set_attraction(&self, response: String) {
            *self.attraction_response.lock().unwrap() = Some(response);
        }
    }

    #[async_trait]
    impl ToolExecutor for MockToolExecutor {
        async fn get_weather(&self, _city: &str) -> Result<String, String> {
            self.weather_response
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| "无模拟天气数据".to_string())
        }

        async fn get_attraction(&self, _city: &str, _weather: &str) -> Result<String, String> {
            self.attraction_response
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| "无模拟景点数据".to_string())
        }
    }

    #[test]
    fn test_parse_action_valid() {
        let (name, args) = parse_action(r#"get_weather(city="北京")"#).unwrap();
        assert_eq!(name, "get_weather");
        assert_eq!(args.get("city").unwrap(), "北京");
    }

    #[test]
    fn test_parse_action_multiple_args() {
        let (name, args) =
            parse_action(r#"get_attraction(city="上海", weather="雨")"#).unwrap();
        assert_eq!(name, "get_attraction");
        assert_eq!(args.get("city").unwrap(), "上海");
        assert_eq!(args.get("weather").unwrap(), "雨");
    }

    #[test]
    fn test_truncate_thought_action() {
        let input = "Thought: 需要查天气\nAction: get_weather(city=\"北京\")\n\nThought: 再查景点\nAction: get_attraction(city=\"北京\", weather=\"晴\")";
        let out = truncate_thought_action(input);
        assert_eq!(out, "Thought: 需要查天气\nAction: get_weather(city=\"北京\")");
    }

    #[tokio::test]
    async fn test_run_agent_with_mock_tools() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "Thought: 先查北京天气\nAction: get_weather(city=\"北京\")"
                    }
                }]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "Thought: 天气晴朗适合户外\nAction: Finish[推荐去故宫]"
                    }
                }]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = DeepSeekClient::new(
            "fake-key".into(),
            mock_server.uri(),
            "deepseek-v4-pro".into(),
        );

        let mock_tools = MockToolExecutor::new();
        mock_tools.set_weather("北京当前天气：晴，气温25℃".to_string());
        mock_tools.set_attraction("推荐故宫".to_string());

        let result = run_agent(&client, &mock_tools, "北京天气及景点").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "推荐去故宫");
    }

    #[tokio::test]
    async fn test_run_agent_with_bad_action() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "Thought: 尝试无效调用\nAction: bad_format"
                    }
                }]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "Thought: 重新尝试\nAction: Finish[无法处理]"
                    }
                }]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = DeepSeekClient::new(
            "fake-key".into(),
            mock_server.uri(),
            "deepseek-v4-pro".into(),
        );
        let mock_tools = MockToolExecutor::new();

        let result = run_agent(&client, &mock_tools, "测试").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "无法处理");
    }

    fn extract_first_thought_action(text: &str) -> Option<&str> {
    let re = Regex::new(r"(?s)(Thought:.*?Action:.*?(?:\n\s*Observation:.*?)?)(?:\n\s*(?:Thought:|Action:)|\z)").unwrap();
        re.captures(text).and_then(|caps| caps.get(1)).map(|m| m.as_str())
    }

    #[test]
    fn test_single_pair() {
        let input = "Thought: 需要查询天气\nAction: get_weather(city=\"北京\")";
        assert_eq!(extract_first_thought_action(input), Some(input));
    }

    #[test]
    fn test_with_observation() {
        let input = "Thought: 需要查询天气\nAction: get_weather(city=\"北京\")\n\nObservation: 北京当前天气：晴\n\nThought: 需要查询天气\nAction: get_weather(city=\"北京\")\n\nObservation: 北京当前天气：晴";
        // let expected = "Thought: 需要查询天气\nAction: get_weather(city=\"北京\")";
        let expected = "Thought: 需要查询天气\nAction: get_weather(city=\"北京\")\n\nObservation: 北京当前天气：晴";
        assert_eq!(extract_first_thought_action(input), Some(expected));
    }

    #[test]
    fn test_multiple_pairs_extracts_first() {
        let input = "Thought: 第一步\nAction: step_one()\n\nThought: 第二步\nAction: step_two()";
        let expected = "Thought: 第一步\nAction: step_one()";
        assert_eq!(extract_first_thought_action(input), Some(expected));
    }

    #[test]
    fn test_end_of_string() {
        let input = "Thought: 最后一步\nAction: finish()";
        assert_eq!(extract_first_thought_action(input), Some(input));
    }

    #[test]
    fn test_no_match() {
        assert_eq!(extract_first_thought_action("没有 Thought-Action 对"), None);
        assert_eq!(extract_first_thought_action("Thought: 缺 Action"), None);
    }

    #[test]
    fn test_user_provided_content() {
        let input = "Thought: 用户要求查询今天北京的天气，然后根据天气推荐景点。我需要先调用 get_weather 工具获取北京今天的天气信息。\n\nAction: get_weather(city=\"北京\")";
        assert_eq!(extract_first_thought_action(input), Some(input));
    }

    #[test]
    fn test_action_followed_by_thought_without_blank_line() {
        // 下一行直接是 Thought: 也会被截断
        let input = "Thought: 查天气\nAction: get_weather(city=\"上海\")\nThought: 下一步";
        let expected = "Thought: 查天气\nAction: get_weather(city=\"上海\")";
        assert_eq!(extract_first_thought_action(input), Some(expected));
    }
}