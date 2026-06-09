use crate::client_message::Message;
use crate::llm_client::LLMClient;
use regex::Regex;
use std::sync::Arc;

const PLANNER_PROMPT_TEMPLATE: &str = r#"
你是一个顶级的AI规划专家。你的任务是将用户提出的复杂问题分解成一个由多个简单步骤组成的行动计划。
请确保计划中的每个步骤都是一个独立的、可执行的子任务，并且严格按照逻辑顺序排列。
你的输出必须是一个Python列表，其中每个元素都是一个描述子任务的字符串。

问题: {question}

请严格按照以下格式输出你的计划，```python与```作为前后缀是必要的:
```python
["步骤1", "步骤2", "步骤3", ...]
"#;

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

pub struct Planner {
    llm_client: Arc<LLMClient>,
}

impl Planner {
    pub fn new(llm_client: Arc<LLMClient>) -> Self {
        Planner { llm_client }
    }

    pub async fn plan(&self, question: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let prompt = PLANNER_PROMPT_TEMPLATE.replace("{question}", question);
        let messages = vec![Message {
            role: "user".to_string(),
            content: prompt,
        }];

        println!("--- 正在生成计划 ---");
        let response_text = self.llm_client.think(messages, 0.0).await?;
        println!("✅ 计划已生成:\n{}", response_text);

        let re = Regex::new(r"python\s*([\s\S]*?)\s*").unwrap();
        let plan_str = match re.captures(&response_text) {
            Some(cap) => cap[1].to_string(),
            None => return Err("未找到计划代码块".into()),
        };

        Self::parse_python_string_list(&plan_str)
    }

    fn parse_python_string_list(input: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let trimmed = input.trim();
        let inner = if trimmed.starts_with('[') && trimmed.ends_with(']') {
            &trimmed[1..trimmed.len() - 1]
        } else {
            return Err("不是列表格式".into());
        };

        let mut steps = Vec::new();
        for part in inner.split(',') {
            let part = part.trim();
            let step = if (part.starts_with('"') && part.ends_with('"'))
                || (part.starts_with('\'') && part.ends_with('\''))
            {
                part[1..part.len() - 1].to_string()
            } else {
                part.to_string()
            };
            steps.push(step);
        }
        Ok(steps)
    }
}

pub struct Executor {
    llm_client: Arc<LLMClient>,
}

impl Executor {
    pub fn new(llm_client: Arc<LLMClient>) -> Self {
        Executor { llm_client }
    }

    pub async fn execute(
        &self,
        question: &str,
        plan: &[String],
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut history = String::new();
        let mut final_answer = String::new();

        println!("\n--- 正在执行计划 ---");
        for (i, step) in plan.iter().enumerate() {
            let step_num = i + 1;
            println!("\n-> 正在执行步骤 {}/{}: {}", step_num, plan.len(), step);

            // 使用 format! 一次构建 Prompt，清晰高效
            let prompt = format!(
                "{} {} {} {} {}",
                EXECUTOR_PROMPT_TEMPLATE,
                question = question,
                plan = format!("{:?}", plan),
                history = if history.is_empty() { "无" } else { &history },
                current_step = step,
            ); // 需要 nightly 或改用位置参数；为兼容，仍用 replace 也可，这里保留原有 replace 方式以保兼容，但逻辑正确。
               // 实际兼容写法：
            let prompt = EXECUTOR_PROMPT_TEMPLATE
                .replace("{question}", question)
                .replace("{plan}", &format!("{:?}", plan))
                .replace(
                    "{history}",
                    if history.is_empty() { "无" } else { &history },
                )
                .replace("{current_step}", step);

            let messages = vec![Message {
                role: "user".to_string(),
                content: prompt,
            }];

            let response_text = self.llm_client.think(messages, 0.0).await?;

            history.push_str(&format!(
                "步骤 {}: {}\n结果: {}\n\n",
                step_num, step, response_text
            ));
            final_answer = response_text;
            println!("✅ 步骤 {} 已完成，结果: {}", step_num, final_answer);
        }
        Ok(final_answer)
    }
}

pub struct PlanAndSolveAgent {
    llm_client: Arc<LLMClient>,
    planner: Planner,
    executor: Executor,
}

impl PlanAndSolveAgent {
    pub fn new(llm_client: Arc<LLMClient>) -> Self {
        let llm_client = llm_client.clone();
        // 克隆客户端供内部使用
        let planner = Planner::new(llm_client.clone());
        let executor = Executor::new(llm_client.clone());
        PlanAndSolveAgent {
            planner,
            executor,
            llm_client, // 保留原始客户端（可选，此处未使用）
        }
    }

    pub async fn run(&self, question: &str) {
        println!("\n--- 开始处理问题 ---\n问题: {}", question);
        let plan = match self.planner.plan(question).await {
            Ok(p) => p,
            Err(e) => {
                println!("\n--- 任务终止 ---\n无法生成有效的行动计划: {}", e);
                return;
            }
        };
        match self.executor.execute(question, &plan).await {
            Ok(final_answer) => println!("\n--- 任务完成 ---\n最终答案: {}", final_answer),
            Err(e) => println!("执行过程中出错: {}", e),
        }
    }
}
