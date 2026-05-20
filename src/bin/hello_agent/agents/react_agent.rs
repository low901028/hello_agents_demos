use crate::hello_agent::core::agent::Agent;
use crate::hello_agent::core::config::Config;
use crate::hello_agent::core::llm::HelloAgentsLlm;
use crate::hello_agent::core::message::{Message, MessageRole};
use crate::hello_agent::core::streaming::{StreamEvent, StreamEventType};
use crate::hello_agent::tools::registry::ToolRegistry;
use crate::hello_agent::tools::tool_filter::ToolFilter;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc;

pub const DEFAULT_REACT_SYSTEM_PROMPT: &str = "你是一个具备推理和行动能力的AI助手。\n\n## 工作流程\n1. Thought工具：记录推理过程\n2. 业务工具：获取信息或执行操作\n3. Finish工具：返回最终答案";

pub struct ReActAgent {
    name: String,
    llm: HelloAgentsLlm,
    tool_registry: Arc<ToolRegistry>,
    system_prompt: String,
    config: Config,
    max_steps: usize,
    builtin_tools: HashSet<String>,
    history: std::sync::RwLock<Vec<Message>>,
}

impl ReActAgent {
    pub fn new(
        name: &str,
        llm: HelloAgentsLlm,
        tool_registry: Arc<ToolRegistry>,
        system_prompt: Option<String>,
        config: Config,
        max_steps: usize,
    ) -> Self {
        ReActAgent {
            name: name.into(),
            llm,
            tool_registry,
            system_prompt: system_prompt.unwrap_or(DEFAULT_REACT_SYSTEM_PROMPT.into()),
            config,
            max_steps,
            builtin_tools: ["Thought", "Finish"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            history: std::sync::RwLock::new(Vec::new()),
        }
    }

    fn build_messages(&self, input: &str) -> Vec<HashMap<String, serde_json::Value>> {
        vec![
            {
                let mut m = HashMap::new();
                m.insert("role".into(), serde_json::json!("system"));
                m.insert("content".into(), serde_json::json!(&self.system_prompt));
                m
            },
            {
                let mut m = HashMap::new();
                m.insert("role".into(), serde_json::json!("user"));
                m.insert("content".into(), serde_json::json!(input));
                m
            },
        ]
    }

    fn build_tool_schemas(&self) -> Vec<HashMap<String, serde_json::Value>> {
        let mut s = vec![serde_json::from_value(serde_json::json!({"type":"function","function":{"name":"Thought","description":"记录推理过程","parameters":{"type":"object","properties":{"reasoning":{"type":"string"}},"required":["reasoning"]}}})).unwrap(), serde_json::from_value(serde_json::json!({"type":"function","function":{"name":"Finish","description":"返回最终答案","parameters":{"type":"object","properties":{"answer":{"type":"string"}},"required":["answer"]}}})).unwrap()];
        s.extend(
            self.tool_registry
                .get_all_tools()
                .iter()
                .map(|t| t.to_openai_schema()),
        );
        s
    }

    fn handle_builtin(
        &self,
        name: &str,
        args: &HashMap<String, serde_json::Value>,
    ) -> (String, bool, Option<String>) {
        match name {
            "Thought" => (
                format!(
                    "推理:{}",
                    args.get("reasoning").and_then(|v| v.as_str()).unwrap_or("")
                ),
                false,
                None,
            ),
            "Finish" => {
                let a = args
                    .get("answer")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                (format!("最终答案:{}", a), true, Some(a))
            }
            _ => ("未知内置工具".into(), false, None),
        }
    }
}

#[async_trait]
impl Agent for ReActAgent {
    fn name(&self) -> &str {
        &self.name
    }
    fn max_steps_mut(&mut self) -> Option<&mut usize> {
        Some(&mut self.max_steps)
    }
    fn max_steps(&self) -> Option<usize> {
        Some(self.max_steps)
    }
    fn run(&self, input: &str) -> String {
        let mut msgs = self.build_messages(input);
        let schemas = self.build_tool_schemas();
        for _step in 1..=self.max_steps {
            let resp = match self.llm.invoke_with_tools(&msgs, &schemas, "auto") {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("LLM错误:{}", e);
                    break;
                }
            };
            if resp.tool_calls.is_empty() {
                let ans = resp.content.unwrap_or("无法回答".into());
                self.history.write().unwrap().push(Message::user(input));
                self.history.write().unwrap().push(Message::assistant(&ans));
                return ans;
            }
            let mut asst = HashMap::new();
            asst.insert("role".into(), serde_json::json!("assistant"));
            asst.insert("content".into(), serde_json::json!(resp.content));
            asst.insert("tool_calls".into(), serde_json::json!(resp.tool_calls.iter().map(|tc| serde_json::json!({"id":tc.id,"type":"function","function":{"name":tc.name,"arguments":tc.arguments}})).collect::<Vec<_>>()));
            msgs.push(asst);
            for tc in &resp.tool_calls {
                let args: HashMap<String, serde_json::Value> =
                    serde_json::from_str(&tc.arguments).unwrap_or_default();
                if self.builtin_tools.contains(&tc.name) {
                    let (content, finished, answer) = self.handle_builtin(&tc.name, &args);
                    msgs.push({
                        let mut m = HashMap::new();
                        m.insert("role".into(), serde_json::json!("tool"));
                        m.insert("tool_call_id".into(), serde_json::json!(&tc.id));
                        m.insert("content".into(), serde_json::json!(&content));
                        m
                    });
                    if finished {
                        let a = answer.unwrap_or_default();
                        self.history.write().unwrap().push(Message::user(input));
                        self.history.write().unwrap().push(Message::assistant(&a));
                        return a;
                    }
                } else {
                    let result = self
                        .tool_registry
                        .execute_tool(&tc.name, &serde_json::to_string(&args).unwrap_or_default())
                        .text;
                    msgs.push({
                        let mut m = HashMap::new();
                        m.insert("role".into(), serde_json::json!("tool"));
                        m.insert("tool_call_id".into(), serde_json::json!(&tc.id));
                        m.insert("content".into(), serde_json::json!(result));
                        m
                    });
                }
            }
        }
        let ans = "无法在限定步数内完成".to_string();
        self.history.write().unwrap().push(Message::user(input));
        self.history.write().unwrap().push(Message::assistant(&ans));
        ans
    }
    fn get_system_prompt(&self) -> Option<&str> {
        Some(&self.system_prompt)
    }
    fn add_message(&mut self, _msg: Message) {}
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
