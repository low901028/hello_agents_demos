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

/// 记忆模块
pub struct Memory {
    records: Vec<serde_json::Value>,
}

impl Memory {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    pub fn add_record(&mut self, record_type: &str, content: &str) {
        self.records.push(serde_json::json!({
            "type": record_type,
            "content": content,
        }));
    }

    pub fn get_last_execution(&self) -> Option<String> {
        self.records
            .iter()
            .rev()
            .find(|r| r["type"] == "execution")
            .and_then(|r| r["content"].as_str())
            .map(|s| s.to_string())
    }
}

/// Reflection Agent
pub struct ReflectionAgent {
    name: String,
    llm: HelloAgentsLLM,
    system_prompt: Option<String>,
    config: Config,
    tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    max_iterations: usize,
    memory: Memory,
    history: Vec<Message>,
}

impl ReflectionAgent {
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
            max_iterations: 3,
            memory: Memory::new(),
            history: Vec::new(),
        }
    }
}

#[async_trait::async_trait]
impl Agent for ReflectionAgent {
    fn name(&self) -> &str { &self.name }
    fn llm(&self) -> &HelloAgentsLLM { &self.llm }
    fn system_prompt(&self) -> Option<&str> { self.system_prompt.as_deref() }
    fn config(&self) -> Option<&Config> { Some(&self.config) }

    async fn run(&mut self, input_text: &str) -> Result<String, HelloAgentsError> {
        // 初始执行
        let initial_prompt = format!("请完成以下任务：\n\n{}", input_text);
        let messages = vec![Message::new(initial_prompt, MessageRole::User)];
        let response = self.llm.invoke(&messages, None, None).await?;
        let mut current = response.content.clone();

        // 迭代反思
        for i in 0..self.max_iterations {
            let reflect_prompt = format!(
                "请审查以下回答：\n\n任务: {}\n\n当前回答: {}\n\n请找出问题并提出改进建议。如果回答已经很好，请回复'无需改进'。",
                input_text, current
            );
            let reflect_messages = vec![Message::new(reflect_prompt, MessageRole::User)];
            let feedback = self.llm.invoke(&reflect_messages, None, None).await?;

            if feedback.content.contains("无需改进") {
                break;
            }

            let refine_prompt = format!(
                "请根据反馈改进回答：\n\n任务: {}\n\n上一轮: {}\n\n反馈: {}",
                input_text, current, feedback.content
            );
            let refine_messages = vec![Message::new(refine_prompt, MessageRole::User)];
            let refined = self.llm.invoke(&refine_messages, None, None).await?;
            current = refined.content;
        }

        self.history.push(Message::new(input_text, MessageRole::User));
        self.history.push(Message::new(current.clone(), MessageRole::Assistant));
        Ok(current)
    }

    async fn run_stream(
        &mut self,
        _input_text: &str,
    ) -> Result<mpsc::UnboundedReceiver<StreamEvent>, HelloAgentsError> {
        Err(HelloAgentsError::Agent("ReflectionAgent run_stream 未实现".into()))
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