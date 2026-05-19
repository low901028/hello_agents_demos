use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::hello_agent::core::agent::Agent;
use crate::hello_agent::core::config::Config;
use crate::hello_agent::core::exceptions::HelloAgentsError;
use crate::hello_agent::core::llm::HelloAgentsLLM;
use crate::hello_agent::core::message::{Message, MessageRole};
use crate::hello_agent::core::streaming::StreamEvent;
use crate::hello_agent::core::lifecycle::LifecycleHook;
use crate::hello_agent::tools::registry::ToolRegistry;

/// Plan and Solve Agent
pub struct PlanSolveAgent {
    name: String,
    llm: HelloAgentsLLM,
    system_prompt: Option<String>,
    config: Config,
    tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    history: Vec<Message>,
}

impl PlanSolveAgent {
    pub fn new(
        name: impl Into<String>,
        llm: HelloAgentsLLM,
        system_prompt: Option<impl Into<String>>,
        config: Config,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    ) -> Self {
        Self {
            name: name.into(),
            llm,
            system_prompt: system_prompt.map(|s| s.into()),
            config,
            tool_registry,
            history: Vec::new(),
        }
    }
}

#[async_trait::async_trait]
impl Agent for PlanSolveAgent {
    fn name(&self) -> &str { &self.name }
    fn llm(&self) -> &HelloAgentsLLM { &self.llm }
    fn system_prompt(&self) -> Option<&str> { self.system_prompt.as_deref() }
    fn config(&self) -> Option<&Config> { Some(&self.config) }

    async fn run(&mut self, input_text: &str) -> Result<String, HelloAgentsError> {
        // 1. 生成计划
        let plan_prompt = format!("请为以下问题生成详细的执行计划：\n\n{}", input_text);
        let plan_messages = vec![Message::new(plan_prompt, MessageRole::User)];
        let plan_response = self.llm.invoke(&plan_messages, None, None).await?;

        // 2. 解析计划（简化：按行分割）
        let plan: Vec<String> = plan_response
            .content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect();

        if plan.is_empty() {
            return Err(HelloAgentsError::Agent("无法生成有效的行动计划".into()));
        }

        // 3. 逐步执行
        let mut results = Vec::new();
        for step in &plan {
            let execute_prompt = format!(
                "原始问题: {}\n\n当前步骤: {}\n\n请执行此步骤并给出结果。",
                input_text, step
            );
            let execute_messages = vec![Message::new(execute_prompt, MessageRole::User)];
            let step_response = self.llm.invoke(&execute_messages, None, None).await?;
            results.push(step_response.content.clone());
        }

        // 4. 汇总答案
        let final_prompt = format!(
            "原始问题: {}\n\n执行结果:\n{}\n\n请给出最终答案。",
            input_text,
            plan.iter().zip(results.iter())
                .map(|(s, r)| format!("{}: {}", s, r))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let final_messages = vec![Message::new(final_prompt, MessageRole::User)];
        let final_response = self.llm.invoke(&final_messages, None, None).await?;

        self.history.push(Message::new(input_text, MessageRole::User));
        self.history.push(Message::new(final_response.content.clone(), MessageRole::Assistant));
        Ok(final_response.content)
    }

    async fn run_stream(
        &mut self,
        _input_text: &str,
    ) -> Result<mpsc::UnboundedReceiver<StreamEvent>, HelloAgentsError> {
        Err(HelloAgentsError::Agent("PlanSolveAgent run_stream 未实现".into()))
    }

    async fn arun(
        &mut self,
        input_text: &str,
        _on_start: Option<LifecycleHook>,
        _on_step: Option<LifecycleHook>,
        _on_finish: Option<LifecycleHook>,
        _on_error: Option<LifecycleHook>,
    ) -> Result<String, HelloAgentsError> {
        self.run(input_text).await
    }

    fn get_history(&self) -> Vec<Message> { self.history.clone() }
    fn clear_history(&mut self) { self.history.clear(); }
}