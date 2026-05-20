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

pub struct Planner {
    llm: HelloAgentsLlm,
    prompt: String,
}

impl Planner {
    pub fn new(llm: HelloAgentsLlm, prompt: Option<String>) -> Self {
        Planner {
            llm,
            prompt: prompt.unwrap_or("你是顶级规划专家，将复杂问题分解为简单步骤".into()),
        }
    }
    pub fn plan(&self, question: &str) -> Vec<String> {
        let tool = serde_json::from_value(serde_json::json!({"type":"function","function":{"name":"generate_plan","description":"生成执行计划","parameters":{"type":"object","properties":{"steps":{"type":"array","items":{"type":"string"}}},"required":["steps"]}}})).unwrap();
        let msgs = vec![
            {
                let mut m = HashMap::new();
                m.insert("role".into(), serde_json::json!("system"));
                m.insert("content".into(), serde_json::json!(&self.prompt));
                m
            },
            {
                let mut m = HashMap::new();
                m.insert("role".into(), serde_json::json!("user"));
                m.insert(
                    "content".into(),
                    serde_json::json!(format!("请为以下问题生成计划:\n\n{}", question)),
                );
                m
            },
        ];
        match self.llm.invoke_with_tools(&msgs, &[tool], "required") {
            Ok(r) => {
                if let Some(tc) = r.tool_calls.first() {
                    if let Ok(args) =
                        serde_json::from_str::<HashMap<String, serde_json::Value>>(&tc.arguments)
                    {
                        return args
                            .get("steps")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|s| s.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                    }
                }
                vec![]
            }
            Err(_) => vec![],
        }
    }
}

pub struct Executor {
    llm: HelloAgentsLlm,
    prompt: String,
}

impl Executor {
    pub fn new(llm: HelloAgentsLlm, prompt: Option<String>) -> Self {
        Executor {
            llm,
            prompt: prompt.unwrap_or("你是顶级执行专家，严格按计划逐步执行".into()),
        }
    }
    pub fn execute(&self, question: &str, plan: &[String]) -> String {
        let mut history: Vec<HashMap<String, String>> = Vec::new();
        let mut answer = String::new();
        for (i, step) in plan.iter().enumerate() {
            let ctx = format!(
                "# 问题:{}\n\n# 计划:\n{}\n\n# 已完成:\n{}\n\n# 当前步骤:{}\n请执行此步骤并给出结果。",
                question,
                plan.iter()
                    .enumerate()
                    .map(|(j, s)| format!("{}. {}", j + 1, s))
                    .collect::<Vec<_>>()
                    .join("\n"),
                if history.is_empty() {
                    "无".into()
                } else {
                    history
                        .iter()
                        .enumerate()
                        .map(|(j, h)| {
                            format!(
                                "步骤{}:{}→{}",
                                j + 1,
                                h.get("step").unwrap_or(&String::new()),
                                h.get("result").unwrap_or(&String::new())
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                },
                step
            );
            let resp = self
                .llm
                .invoke(&[
                    {
                        let mut m = HashMap::new();
                        m.insert("role".into(), serde_json::json!("system"));
                        m.insert("content".into(), serde_json::json!(&self.prompt));
                        m
                    },
                    {
                        let mut m = HashMap::new();
                        m.insert("role".into(), serde_json::json!("user"));
                        m.insert("content".into(), serde_json::json!(ctx));
                        m
                    },
                ])
                .map(|r| r.content)
                .unwrap_or_else(|e| format!("错误:{}", e));
            let mut h = HashMap::new();
            h.insert("step".into(), step.clone());
            h.insert("result".into(), resp.clone());
            history.push(h);
            answer = resp;
        }
        answer
    }
}

pub struct PlanSolveAgent {
    name: String,
    planner: Planner,
    executor: Executor,
    history: std::sync::RwLock<Vec<Message>>,
}

impl PlanSolveAgent {
    pub fn new(
        name: &str,
        llm: HelloAgentsLlm,
        _system_prompt: Option<String>,
        _config: Config,
        planner_prompt: Option<String>,
        executor_prompt: Option<String>,
        _tool_registry: Option<Arc<ToolRegistry>>,
        _enable_tools: bool,
        _max_tool_iter: usize,
    ) -> Self {
        PlanSolveAgent {
            name: name.into(),
            planner: Planner::new(llm.clone(), planner_prompt),
            executor: Executor::new(llm, executor_prompt),
            history: std::sync::RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl Agent for PlanSolveAgent {
    fn name(&self) -> &str {
        &self.name
    }
    fn run(&self, input: &str) -> String {
        let plan = self.planner.plan(input);
        if plan.is_empty() {
            let a = "无法生成计划".to_string();
            self.history.write().unwrap().push(Message::user(input));
            self.history.write().unwrap().push(Message::assistant(&a));
            return a;
        }
        let answer = self.executor.execute(input, &plan);
        self.history.write().unwrap().push(Message::user(input));
        self.history
            .write()
            .unwrap()
            .push(Message::assistant(&answer));
        answer
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
