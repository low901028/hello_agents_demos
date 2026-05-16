use anyhow::{Context, Result};
use futures::StreamExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::Write;
use crate::simple_agent::simple_agent_client::{HelloAgentsLLM, Message};
// ==================== 1. HelloAgentsLLM ====================

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
            name: None,
        }];

        println!("--- 正在生成计划 ---");
        let response_text = match self.llm_client.think(messages, 0.0, Some(true)).await {
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
                name: None,
            }];

            let response_text = match self.llm_client.think(messages, 0.0, Some(true)).await {
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

pub struct PlanAndSolveAgent {
    planner: Planner,
    executor: Executor,
}

impl PlanAndSolveAgent {
    pub fn new(planner_llm: HelloAgentsLLM, executor_llm: HelloAgentsLLM) -> Self {
        let planner = Planner::new(planner_llm);
        let executor = Executor::new(executor_llm);
        Self { planner, executor }
    }

    pub async fn run(&self, question: &str) {
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
