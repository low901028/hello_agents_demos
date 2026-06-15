// ============================================================
// agents/reflection.rs
// Reflection Agent – 自我反思与迭代优化
// ============================================================

use crate::core::agent_runtime::AgentRuntime;
use crate::core::traits::agent::Agent;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::message::Message;
use async_trait::async_trait;

pub struct ReflectionAgent {
    name: String,
    system_prompt: Option<String>,
    max_iterations: usize,
}

impl ReflectionAgent {
    pub fn new(name: &str, system_prompt: Option<String>, max_iterations: usize) -> Self {
        Self {
            name: name.into(),
            system_prompt,
            max_iterations,
        }
    }
}

#[async_trait]
impl Agent for ReflectionAgent {
    fn name(&self) -> &str {
        &self.name
    }

    async fn run(
        &mut self,
        input: &str,
        runtime: &AgentRuntime,
    ) -> Result<String, HelloAgentError> {
        let sys = self
            .system_prompt
            .clone()
            .unwrap_or_else(|| "你是一个善于反思的助手".into());

        let mut current = String::new();

        for i in 0..self.max_iterations {
            let prompt = if i == 0 {
                format!("{sys}\n\n任务：{input}")
            } else {
                format!("{sys}\n\n这是上一轮的回答：\n{current}\n\n请反思并改进：")
            };
            let messages = vec![Message::user(&prompt)];
            let resp = runtime.llm.chat(&messages, None, None).await?;
            current = resp.content.unwrap_or_default();
        }

        Ok(current)
    }
}
