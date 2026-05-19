use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::hello_agent::core::agent::Agent;
use crate::hello_agent::core::config::Config;
use crate::hello_agent::core::exceptions::HelloAgentsError;
use crate::hello_agent::core::llm::HelloAgentsLLM;
use crate::hello_agent::core::message::{Message, MessageRole};
use crate::hello_agent::tools::registry::ToolRegistry;

/// 简单对话 Agent
pub struct SimpleAgent {
    name: String,
    llm: HelloAgentsLLM,
    system_prompt: Option<String>,
    config: Config,
    tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    enable_tool_calling: bool,
    max_tool_iterations: usize,
    history: Vec<Message>,
}

impl SimpleAgent {
    pub fn new(
        name: impl Into<String>,
        llm: HelloAgentsLLM,
        system_prompt: Option<impl Into<String>>,
        config: Config,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    ) -> Self {
        let enable = tool_registry.is_some();
        Self {
            name: name.into(),
            llm,
            system_prompt: system_prompt.map(|s| s.into()),
            config,
            tool_registry,
            enable_tool_calling: enable,
            max_tool_iterations: 3,
            history: Vec::new(),
        }
    }

    fn build_tool_schemas(&self) -> Vec<serde_json::Value> {
        // 简化：返回空，实际应从 tool_registry 获取
        Vec::new()
    }
}

#[async_trait::async_trait]
impl Agent for SimpleAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn llm(&self) -> &HelloAgentsLLM {
        &self.llm
    }

    fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    fn config(&self) -> Option<&Config> {
        Some(&self.config)
    }

    async fn run(&mut self, input_text: &str) -> Result<String, HelloAgentsError> {
        let mut messages = Vec::new();

        if let Some(ref sys) = self.system_prompt {
            messages.push(Message::new(sys.clone(), MessageRole::System));
        }

        for msg in &self.history {
            messages.push(msg.clone());
        }

        messages.push(Message::new(input_text, MessageRole::User));

        let response = self.llm.invoke(&messages, None, None).await?;
        let response_text = response.content.clone();

        self.history.push(Message::new(input_text, MessageRole::User));
        self.history.push(Message::new(response_text.clone(), MessageRole::Assistant));

        Ok(response_text)
    }

    async fn run_stream(
        &mut self,
        _input_text: &str,
    ) -> Result<mpsc::UnboundedReceiver<crate::hello_agent::core::streaming::StreamEvent>, HelloAgentsError> {
        Err(HelloAgentsError::Agent("SimpleAgent run_stream 未实现".into()))
    }

    async fn arun(
        &mut self,
        input_text: &str,
        _on_start: Option<crate::hello_agent::core::lifecycle::LifecycleHook>,
        _on_step: Option<crate::hello_agent::core::lifecycle::LifecycleHook>,
        _on_finish: Option<crate::hello_agent::core::lifecycle::LifecycleHook>,
        _on_error: Option<crate::hello_agent::core::lifecycle::LifecycleHook>,
    ) -> Result<String, HelloAgentsError> {
        self.run(input_text).await
    }

    fn get_history(&self) -> Vec<Message> {
        self.history.clone()
    }

    fn clear_history(&mut self) {
        self.history.clear();
    }
}