use crate::hello_agent::core::agent::{Agent, AgentBase};
use crate::hello_agent::core::config::Config;
use crate::hello_agent::core::llm::HelloAgentsLlm;
use crate::hello_agent::core::message::{Message, MessageRole};
use crate::hello_agent::core::streaming::{StreamEvent, StreamEventType};
use crate::hello_agent::tools::registry::ToolRegistry;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct SimpleAgent {
    base: AgentBase,
    enable_tool_calling: bool,
    max_tool_iterations: usize,
}

impl SimpleAgent {
    pub fn new(
        name: &str,
        llm: HelloAgentsLlm,
        system_prompt: Option<String>,
        config: Config,
        tool_registry: Option<Arc<ToolRegistry>>,
        enable_tool_calling: bool,
        max_tool_iterations: usize,
    ) -> Self {
        SimpleAgent {
            base: AgentBase::new(name, llm, system_prompt, Some(config), tool_registry),
            enable_tool_calling,
            max_tool_iterations,
        }
    }

    fn build_messages(&self, input: &str) -> Vec<HashMap<String, serde_json::Value>> {
        let mut msgs = Vec::new();
        if let Some(ref sp) = self.base.system_prompt {
            msgs.push({
                let mut m = HashMap::new();
                m.insert("role".into(), serde_json::json!("system"));
                m.insert("content".into(), serde_json::json!(sp));
                m
            });
        }
        for msg in &self.base.get_history() {
            msgs.push(msg.to_openai_dict());
        }
        msgs.push({
            let mut m = HashMap::new();
            m.insert("role".into(), serde_json::json!("user"));
            m.insert("content".into(), serde_json::json!(input));
            m
        });
        msgs
    }
}

#[async_trait]
impl Agent for SimpleAgent {
    fn name(&self) -> &str {
        &self.base.name
    }
    fn run(&self, input: &str) -> String {
        let msgs = self.build_messages(input);
        if !self.enable_tool_calling || self.base.tool_registry.is_none() {
            match self.base.llm.invoke(&msgs) {
                Ok(r) => {
                    let t = r.content.clone();
                    self.base.add_message(Message::user(input));
                    self.base.add_message(Message::assistant(&t));
                    t
                }
                Err(e) => format!("错误:{}", e),
            }
        } else {
            let schemas = self.base.build_tool_schemas();
            let mut cur = msgs;
            let mut final_resp = String::new();
            for _ in 0..self.max_tool_iterations {
                match self.base.llm.invoke_with_tools(&cur, &schemas, "auto") {
                    Ok(r) => {
                        if r.tool_calls.is_empty() {
                            final_resp = r.content.unwrap_or_default();
                            break;
                        }
                        let mut asst = HashMap::new();
                        asst.insert("role".into(), serde_json::json!("assistant"));
                        asst.insert("content".into(), serde_json::json!(r.content));
                        asst.insert("tool_calls".into(), serde_json::json!(r.tool_calls.iter().map(|tc| serde_json::json!({"id":tc.id,"type":"function","function":{"name":tc.name,"arguments":tc.arguments}})).collect::<Vec<_>>()));
                        cur.push(asst);
                        for tc in &r.tool_calls {
                            let args: HashMap<String, serde_json::Value> =
                                serde_json::from_str(&tc.arguments).unwrap_or_default();
                            let result = self.base.execute_tool_call(&tc.name, &args);
                            cur.push({
                                let mut m = HashMap::new();
                                m.insert("role".into(), serde_json::json!("tool"));
                                m.insert("tool_call_id".into(), serde_json::json!(&tc.id));
                                m.insert("content".into(), serde_json::json!(result));
                                m
                            });
                        }
                    }
                    Err(e) => {
                        final_resp = format!("错误:{}", e);
                        break;
                    }
                }
            }
            if final_resp.is_empty() {
                if let Ok(r) = self.base.llm.invoke(&cur) {
                    final_resp = r.content;
                }
            }
            self.base.add_message(Message::user(input));
            self.base.add_message(Message::assistant(&final_resp));
            final_resp
        }
    }
    fn get_system_prompt(&self) -> Option<&str> {
        self.base.system_prompt.as_deref()
    }
    fn add_message(&mut self, msg: Message) {
        self.base.add_message(msg);
    }
    fn get_history(&self) -> Vec<Message> {
        self.base.get_history()
    }
    fn clear_history(&mut self) {
        self.base.clear_history();
    }
    async fn arun_stream(&self, input: &str) -> mpsc::Receiver<StreamEvent> {
        let (tx, rx) = mpsc::channel(100);
        let name = self.name().to_string();
        let inp = input.to_string();
        tokio::spawn(async move {
            let _ = tx
                .send(StreamEvent::with_data(
                    StreamEventType::AgentStart,
                    &name,
                    "input_text",
                    inp,
                ))
                .await;
            let _ = tx
                .send(StreamEvent::with_data(
                    StreamEventType::AgentFinish,
                    &name,
                    "result",
                    "done",
                ))
                .await;
        });
        rx
    }
}
