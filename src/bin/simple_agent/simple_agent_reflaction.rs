use anyhow::{Context, Result};
use dotenvy::dotenv;
use std::env;
use crate::simple_agent::simple_agent_client::{HelloAgentsLLM, Message};
// ==================== 记忆模块 ====================

#[derive(Clone, Debug)]
enum RecordType {
    Execution,
    Reflection,
}

impl RecordType {
    fn as_str(&self) -> &'static str {
        match self {
            RecordType::Execution => "execution",
            RecordType::Reflection => "reflection",
        }
    }
}

struct Record {
    record_type: RecordType,
    content: String,
}

struct Memory {
    records: Vec<Record>,
}

impl Memory {
    fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    fn add_record(&mut self, record_type: RecordType, content: String) {
        println!("📝 记忆已更新，新增一条 '{}' 记录。", record_type.as_str());
        self.records.push(Record {
            record_type,
            content,
        });
    }

    fn get_trajectory(&self) -> String {
        let mut trajectory = String::new();
        for record in &self.records {
            match record.record_type {
                RecordType::Execution => {
                    trajectory.push_str(&format!(
                        "--- 上一轮尝试 (代码) ---\n{}\n\n",
                        record.content
                    ));
                }
                RecordType::Reflection => {
                    trajectory.push_str(&format!("--- 评审员反馈 ---\n{}\n\n", record.content));
                }
            }
        }
        trajectory.trim().to_string()
    }

    fn get_last_execution(&self) -> Option<&str> {
        self.records
            .iter()
            .rev()
            .find(|r| matches!(r.record_type, RecordType::Execution))
            .map(|r| r.content.as_str())
    }
}

// ==================== ReflectionAgent ====================

const INITIAL_PROMPT_TEMPLATE: &str = r#"
你是一位资深的Python程序员。请根据以下要求，编写一个Python函数。
你的代码必须包含完整的函数签名、文档字符串，并遵循PEP 8编码规范。

要求: {task}

请直接输出代码，不要包含任何额外的解释。
"#;

const REFLECT_PROMPT_TEMPLATE: &str = r#"
你是一位极其严格的代码评审专家和资深算法工程师，对代码的性能有极致的要求。
你的任务是审查以下Python代码，并专注于找出其在**算法效率**上的主要瓶颈。

# 原始任务:
{task}

# 待审查的代码:
```python
{code}
```
请分析该代码的时间复杂度，并思考是否存在一种**算法上更优**的解决方案来显著提升性能。
如果存在，请清晰地指出当前算法的不足，并提出具体的、可行的改进算法建议（例如，使用筛法替代试除法）。
如果代码在算法层面已经达到最优，才能回答“无需改进”。

请直接输出你的反馈，不要包含任何额外的解释。
"#;

const REFINE_PROMPT_TEMPLATE: &str = r#"
你是一位资深的Python程序员。你正在根据一位代码评审专家的反馈来优化你的代码。

# 原始任务:
{task}

# 你上一轮尝试的代码:
{last_code_attempt}

# 评审员的反馈:
{feedback}

请根据评审员的反馈，生成一个优化后的新版本代码。
你的代码必须包含完整的函数签名、文档字符串，并遵循PEP 8编码规范。
请直接输出优化后的代码，不要包含任何额外的解释。
"#;

pub struct ReflectionAgent {
    llm_client: HelloAgentsLLM, // 直接使用已有的客户端类型
    memory: Memory,
    max_iterations: usize,
}

impl ReflectionAgent {
    pub fn new(llm_client: HelloAgentsLLM, max_iterations: usize) -> Self {
        Self {
            llm_client,
            memory: Memory::new(),
            max_iterations,
        }
    }

    pub async fn run(&mut self, task: &str) -> Result<String> {
        println!("\n--- 开始处理任务 ---\n任务: {}", task);

        // 1. 初始执行
        println!("\n--- 正在进行初始尝试 ---");
        let initial_prompt = INITIAL_PROMPT_TEMPLATE.replace("{task}", task);
        let initial_code = self.call_llm(&initial_prompt).await?;
        self.memory.add_record(RecordType::Execution, initial_code);

        // 2. 迭代反思与优化
        for i in 0..self.max_iterations {
            println!("\n--- 第 {}/{} 轮迭代 ---", i + 1, self.max_iterations);

            // a. 反思
            println!("\n-> 正在进行反思...");
            let last_code = self.memory.get_last_execution().unwrap_or("").to_string();
            let reflect_prompt = REFLECT_PROMPT_TEMPLATE
                .replace("{task}", task)
                .replace("{code}", &last_code);
            let feedback = self.call_llm(&reflect_prompt).await?;
            self.memory
                .add_record(RecordType::Reflection, feedback.clone());

            // b. 检查停止条件
            if feedback.contains("无需改进")
                || feedback.to_lowercase().contains("no need for improvement")
            {
                println!("\n✅ 反思认为代码已无需改进，任务完成。");
                break;
            }

            // c. 优化
            println!("\n-> 正在进行优化...");
            let refine_prompt = REFINE_PROMPT_TEMPLATE
                .replace("{task}", task)
                .replace("{last_code_attempt}", &last_code)
                .replace("{feedback}", &feedback);
            let refined_code = self.call_llm(&refine_prompt).await?;
            self.memory.add_record(RecordType::Execution, refined_code);
        }

        let final_code = self.memory.get_last_execution().unwrap_or("").to_string();
        println!("\n--- 任务完成 ---\n最终生成的代码:\n{}", final_code);
        Ok(final_code)
    }

    /// 调用 LLM 的辅助方法，避免重复构造 Message
    async fn call_llm(&self, prompt: &str) -> Result<String> {
        let messages = vec![Message {
            role: "user".into(),
            content: prompt.to_string(),
            ..Default::default()
        }];
        self.llm_client.think(messages, 0.0, Some(true)).await
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_add_and_retrieve() {
        let mut mem = Memory::new();
        mem.add_record(RecordType::Execution, "print('hello')".to_string());
        mem.add_record(RecordType::Reflection, "needs optimization".to_string());

        assert_eq!(mem.get_last_execution(), Some("print('hello')"));
        let trajectory = mem.get_trajectory();
        assert!(trajectory.contains("上一轮尝试"));
        assert!(trajectory.contains("评审员反馈"));
    }

    #[test]
    fn test_memory_empty() {
        let mem = Memory::new();
        assert_eq!(mem.get_last_execution(), None);
        assert_eq!(mem.get_trajectory(), "");
    }
}
