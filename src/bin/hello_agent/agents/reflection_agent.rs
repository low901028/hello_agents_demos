use crate::hello_agent::core::agent::Agent;
use crate::hello_agent::core::config::Config;
use crate::hello_agent::core::llm::HelloAgentsLlm;
use crate::hello_agent::core::message::{Message, MessageRole};
use crate::hello_agent::core::streaming::{StreamEvent, StreamEventType};
use crate::hello_agent::tools::registry::ToolRegistry;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct Memory {
    records: Vec<(String, String)>,
}

impl Memory {
    pub fn new() -> Self {
        Memory {
            records: Vec::new(),
        }
    }
    fn add(&mut self, t: &str, c: &str) {
        self.records.push((t.into(), c.into()));
    }
    fn last_execution(&self) -> String {
        self.records
            .iter()
            .rev()
            .find(|(t, _)| t == "execution")
            .map(|(_, c)| c.clone())
            .unwrap_or_default()
    }
}

pub struct ReflectionAgent {
    name: String,
    llm: HelloAgentsLlm,
    system_prompt: String,
    config: Config,
    max_iterations: usize,
    history: std::sync::RwLock<Vec<Message>>,
}

impl ReflectionAgent {
    pub fn new(
        name: &str,
        llm: HelloAgentsLlm,
        system_prompt: Option<String>,
        config: Config,
        max_iterations: usize,
        _tool_registry: Option<Arc<ToolRegistry>>,
        _enable_tools: bool,
        _max_tool_iter: usize,
    ) -> Self {
        ReflectionAgent { name: name.into(), llm, system_prompt: system_prompt.unwrap_or("你是具有自我反思能力的AI助手。\n1.先完成任务\n2.反思回答\n3.优化回答\n4.如果够好回复'无需改进'".into()), config, max_iterations, history: std::sync::RwLock::new(Vec::new()) }
    }

    fn call_llm(&self, msgs: &[HashMap<String, serde_json::Value>]) -> String {
        self.llm
            .invoke(msgs)
            .map(|r| r.content)
            .unwrap_or_else(|e| format!("错误:{}", e))
    }
}

#[async_trait]
impl Agent for ReflectionAgent {
    fn name(&self) -> &str {
        &self.name
    }
    fn run(&self, input: &str) -> String {
        let mut mem = Memory::new();
        let initial = self.call_llm(&[
            {
                let mut m = HashMap::new();
                m.insert("role".into(), serde_json::json!("system"));
                m.insert("content".into(), serde_json::json!(&self.system_prompt));
                m
            },
            {
                let mut m = HashMap::new();
                m.insert("role".into(), serde_json::json!("user"));
                m.insert(
                    "content".into(),
                    serde_json::json!(format!("请完成任务:\n\n{}", input)),
                );
                m
            },
        ]);
        mem.add("execution", &initial);
        for _i in 0..self.max_iterations {
            let last = mem.last_execution();
            let feedback = self.call_llm(&[
                {
                    let mut m = HashMap::new();
                    m.insert("role".into(), serde_json::json!("system"));
                    m.insert("content".into(), serde_json::json!(&self.system_prompt));
                    m
                },
                {
                    let mut m = HashMap::new();
                    m.insert("role".into(), serde_json::json!("user"));
                    m.insert(
                        "content".into(),
                        serde_json::json!(format!(
                            "审查回答:\n\n任务:{}\n\n回答:{}\n\n指出不足或回复'无需改进'",
                            input, last
                        )),
                    );
                    m
                },
            ]);
            mem.add("reflection", &feedback);
            if feedback.contains("无需改进") {
                break;
            }
            let refined = self.call_llm(&[
                {
                    let mut m = HashMap::new();
                    m.insert("role".into(), serde_json::json!("system"));
                    m.insert("content".into(), serde_json::json!(&self.system_prompt));
                    m
                },
                {
                    let mut m = HashMap::new();
                    m.insert("role".into(), serde_json::json!("user"));
                    m.insert(
                        "content".into(),
                        serde_json::json!(format!(
                            "根据反馈改进:\n\n任务:{}\n\n上轮:{}\n\n反馈:{}\n\n请给改进后的回答",
                            input, last, feedback
                        )),
                    );
                    m
                },
            ]);
            mem.add("execution", &refined);
        }
        let final_res = mem.last_execution();
        self.history.write().unwrap().push(Message::user(input));
        self.history
            .write()
            .unwrap()
            .push(Message::assistant(&final_res));
        final_res
    }
    fn get_system_prompt(&self) -> Option<&str> {
        Some(&self.system_prompt)
    }
    fn add_message(&mut self, _: Message) {}
    fn get_history(&self) -> Vec<Message> {
        self.history.read().unwrap().clone()
    }
    fn clear_history(&mut self) {
        self.history.write().unwrap().clear();
    }
    async fn arun_stream(&self, input: &str) -> mpsc::Receiver<StreamEvent> {
        let (tx, rx) = mpsc::channel(100);
        let name = self.name.clone();
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
