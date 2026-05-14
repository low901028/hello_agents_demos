use anyhow::{Context, Result};
use dotenvy::dotenv;
use futures::StreamExt;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::Write;
use std::time::Duration;

// ==================== 1. HelloAgentsLLM ====================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f64,
    stream: bool,
}

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

pub struct HelloAgentsLLM {
    model: String,
    api_key: String,
    base_url: String,
    client: Client,
}

impl HelloAgentsLLM {
    pub fn new(
        model: Option<&str>,
        api_key: Option<&str>,
        base_url: Option<&str>,
        timeout_secs: Option<u64>,
    ) -> Result<Self> {
        // 加载 .env 文件（忽略错误，但打印警告）
        if let Err(e) = dotenv() {
            eprintln!("警告：加载 .env 文件时出错: {}", e);
        }

        let model = model
            .map(|s| s.to_string())
            .or_else(|| env::var("LLM_MODEL_ID").ok())
            .context("模型 ID 未提供，且环境变量 LLM_MODEL_ID 未设置")?;

        let api_key = api_key
            .map(|s| s.to_string())
            .or_else(|| env::var("LLM_API_KEY").ok())
            .context("API 密钥未提供，且环境变量 LLM_API_KEY 未设置")?;

        let base_url = base_url
            .map(|s| s.to_string())
            .or_else(|| env::var("LLM_BASE_URL").ok())
            .context("服务地址未提供，且环境变量 LLM_BASE_URL 未设置")?;

        let timeout = timeout_secs
            .or_else(|| env::var("LLM_TIMEOUT").ok().and_then(|v| v.parse().ok()))
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

    /// 调用 LLM 思考，返回完整响应文本（流式输出）
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

            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                        if let Some(first_choice) = chunk.choices.first() {
                            if let Some(content) = &first_choice.delta.content {
                                print!("{}", content);
                                std::io::stdout().flush().ok();
                                collected.push_str(content);
                            }
                        }
                    }
                }
            }
        }

        println!();
        Ok(collected)
    }
}

// ==================== 2. Python 列表解析工具 ====================

/// 从 LLM 返回的文本中解析 Python 列表（字符串元素）
fn parse_python_list(text: &str) -> Result<Vec<String>> {
    // 提取 ```python ... ``` 代码块中的内容
    let re_code = Regex::new(r"```python\s*(.*?)\s*```").unwrap();
    let list_str = re_code
        .captures(text)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().trim())
        .context("未找到 ```python ... ``` 代码块")?;

    // 提取最外层方括号内的内容
    let re_brackets = Regex::new(r"^\s*\[(.*)\]\s*$").unwrap();
    let inner = re_brackets
        .captures(list_str)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str())
        .context("列表格式无效（未找到方括号）")?;

    // 按逗号分割，处理字符串引号
    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut in_escape = false;

    for ch in inner.chars() {
        if in_escape {
            current.push(ch);
            in_escape = false;
            continue;
        }
        if ch == '\\' {
            current.push(ch);
            in_escape = true;
            continue;
        }
        if ch == '"' || ch == '\'' {
            in_string = !in_string;
            continue; // 跳过引号本身
        }
        if !in_string && ch == ',' {
            items.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if !current.trim().is_empty() {
        items.push(current.trim().to_string());
    }

    Ok(items)
}

// ==================== 3. Planner ====================

const PLANNER_PROMPT_TEMPLATE: &str = r#"
你是一个顶级的AI规划专家。你的任务是将用户提出的复杂问题分解成一个由多个简单步骤组成的行动计划。
请确保计划中的每个步骤都是一个独立的、可执行的子任务，并且严格按照逻辑顺序排列。
你的输出必须是一个Python列表，其中每个元素都是一个描述子任务的字符串。

问题: {question}

请严格按照以下格式输出你的计划，```python与```作为前后缀是必要的:
```python
["步骤1", "步骤2", "步骤3", ...]
```
"#;

struct Planner {
    llm_client: HelloAgentsLLM,
}

impl Planner {
    fn new(llm_client: HelloAgentsLLM) -> Self {
        Self { llm_client }
    }

    async fn plan(&self, question: &str) -> Vec<String> {
        let prompt = PLANNER_PROMPT_TEMPLATE.replace("{question}", question);
        let messages = vec![Message {
            role: "user".into(),
            content: prompt,
        }];

        println!("--- 正在生成计划 ---");
        let response_text = match self.llm_client.think(messages, 0.0).await {
            Ok(text) => text,
            Err(e) => {
                eprintln!("❌ 调用LLM失败: {}", e);
                return vec![];
            }
        };
        println!("✅ 计划已生成:\n{}", response_text);

        match parse_python_list(&response_text) {
            Ok(plan) => plan,
            Err(e) => {
                eprintln!("❌ 解析计划时出错: {}", e);
                eprintln!("原始响应: {}", response_text);
                vec![]
            }
        }
    }
}

// ==================== 4. Executor ====================

const EXECUTOR_PROMPT_TEMPLATE: &str = r#"
你是一位顶级的AI执行专家。你的任务是严格按照给定的计划，一步步地解决问题。
你将收到原始问题、完整的计划、以及到目前为止已经完成的步骤和结果。
请你专注于解决“当前步骤”，并仅输出该步骤的最终答案，不要输出任何额外的解释或对话。

原始问题:
{question}

完整计划:
{plan}

历史步骤与结果:
{history}

当前步骤:
{current_step}

请仅输出针对“当前步骤”的回答:
"#;

struct Executor {
    llm_client: HelloAgentsLLM,
}

impl Executor {
    fn new(llm_client: HelloAgentsLLM) -> Self {
        Self { llm_client }
    }

    async fn execute(&self, question: &str, plan: &[String]) -> String {
        let mut history = String::new();
        let mut final_answer = String::new();

        println!("\n--- 正在执行计划 ---");
        for (i, step) in plan.iter().enumerate() {
            let step_num = i + 1;
            println!("\n-> 正在执行步骤 {}/{}: {}", step_num, plan.len(), step);

            let prompt = EXECUTOR_PROMPT_TEMPLATE
                .replace("{question}", question)
                .replace("{plan}", &format!("{:?}", plan)) // 使用 Debug 格式生成类似 ["..."] 的表示
                .replace(
                    "{history}",
                    if history.is_empty() { "无" } else { &history },
                )
                .replace("{current_step}", step);

            let messages = vec![Message {
                role: "user".into(),
                content: prompt,
            }];

            let response_text = match self.llm_client.think(messages, 0.0).await {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("❌ 步骤 {} 执行失败: {}", step_num, e);
                    "执行失败".to_string()
                }
            };

            history.push_str(&format!(
                "步骤 {}: {}\n结果: {}\n\n",
                step_num, step, response_text
            ));
            final_answer = response_text;
            println!("✅ 步骤 {} 已完成，结果: {}", step_num, final_answer);
        }

        final_answer
    }
}

// ==================== 5. PlanAndSolveAgent ====================

struct PlanAndSolveAgent {
    planner: Planner,
    executor: Executor,
}

impl PlanAndSolveAgent {
    fn new(planner_llm: HelloAgentsLLM, executor_llm: HelloAgentsLLM) -> Self {
        let planner = Planner::new(planner_llm);
        let executor = Executor::new(executor_llm);
        Self { planner, executor }
    }

    async fn run(&self, question: &str) {
        println!("\n--- 开始处理问题 ---\n问题: {}", question);
        let plan = self.planner.plan(question).await;
        if plan.is_empty() {
            println!("\n--- 任务终止 --- \n无法生成有效的行动计划。");
            return;
        }
        let final_answer = self.executor.execute(question, &plan).await;
        println!("\n--- 任务完成 ---\n最终答案: {}", final_answer);
    }
}

// ==================== 6. 主函数 ====================

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    // 初始化环境变量（打印警告而非中断）
    if let Err(e) = dotenv() {
        eprintln!("警告：加载 .env 文件时出错: {}", e);
    }

    // 创建两个 LLM 客户端（实际配置相同，但需要两份所有权）
    // 为了避免复杂性，我们创建一个，并用 Arc<Mutex<>> 或直接创建两个实例。
    // 由于 HelloAgentsLLM 包含 reqwest::Client（内部是 Arc），复制配置是简单的。
    let model = "deepseek-v4-flash";
    let api_key = env::var("DEEPSEEK_API_KEY").context("请设置 DEEPSEEK_API_KEY")?;
    let base_url = "https://api.deepseek.com";
    let timeout = env::var("LLM_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);

    let planner_llm =
        HelloAgentsLLM::new(Some(&model), Some(&api_key), Some(&base_url), Some(timeout))?;
    let executor_llm =
        HelloAgentsLLM::new(Some(&model), Some(&api_key), Some(&base_url), Some(timeout))?;

    let agent = PlanAndSolveAgent::new(planner_llm, executor_llm);

    //let question = "一个水果店周一卖出了15个苹果。周二卖出的苹果数量是周一的两倍。周三卖出的数量比周二少了5个。请问这三天总共卖出了多少个苹果？";
    let question = r#"我司目前每日新增数据25-30TB,为了满足日常数据分析，统计，模型训练，AI智能化的需求，需要建设一个满足多模态大数据平台；
           帮我整理一份需求，设计, 实施(包括技术选型，运维成本，用户使用成本等)
    "#;
    agent.run(question).await;

    Ok(())
}

// ==================== 7. 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_python_list_normal() {
        let text = r#"```python
["步骤1", "步骤2", "步骤3"]

"#;
        let plan = parse_python_list(text).unwrap();
        assert_eq!(plan, vec!["步骤1", "步骤2", "步骤3"]);
    }

    #[test]
    fn test_parse_python_list_with_newlines() {
        let text = "```python\n[\"研究目标\", \"收集数据\"]\n```";
        let plan = parse_python_list(text).unwrap();
        assert_eq!(plan, vec!["研究目标", "收集数据"]);
    }

    #[test]
    fn test_parse_invalid_format() {
        let text = "没有代码块";
        assert!(parse_python_list(text).is_err());
    }
}
