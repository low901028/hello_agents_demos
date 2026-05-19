use std::collections::HashMap;
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
use crate::hello_agent::core::adapter::base::ToolChoice;

const DEFAULT_REACT_SYSTEM_PROMPT: &str = r#"你是一个具备推理和行动能力的 AI 助手。

## 工作流程
你可以通过调用工具来完成任务：

1. **Thought 工具**：用于记录你的推理过程和分析
2. **业务工具**：用于获取信息或执行操作
3. **Finish 工具**：用于返回最终答案
"#;

/// ReAct Agent
pub struct ReActAgent {
    name: String,
    llm: HelloAgentsLLM,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    system_prompt: Option<String>,
    config: Option<Config>,
    max_steps: usize,
    builtin_tools: std::collections::HashSet<String>,
    history: Vec<Message>,
}

impl ReActAgent {
    pub fn new(
        name: impl Into<String>,
        llm: HelloAgentsLLM,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        system_prompt: Option<impl Into<String>>,
        config: Option<Config>,
    ) -> Self {
        let mut builtin = std::collections::HashSet::new();
        builtin.insert("Thought".into());
        builtin.insert("Finish".into());

        Self {
            name: name.into(),
            llm,
            tool_registry: tool_registry.unwrap_or_else(|| Arc::new(Mutex::new(ToolRegistry::new(None)))),
            system_prompt: system_prompt.map(|s| s.into()).or_else(|| Some(DEFAULT_REACT_SYSTEM_PROMPT.into())),
            config,
            max_steps: 5,
            builtin_tools: builtin,
            history: Vec::new(),
        }
    }

    fn build_tool_schemas(&self) -> Vec<serde_json::Value> {
        let mut schemas = vec![
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "Thought",
                    "description": "分析问题，制定策略，记录推理过程",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "reasoning": {"type": "string", "description": "你的推理过程和分析"}
                        },
                        "required": ["reasoning"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "Finish",
                    "description": "当你有足够信息得出结论时，使用此工具返回最终答案",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "answer": {"type": "string", "description": "最终答案"}
                        },
                        "required": ["answer"]
                    }
                }
            }),
        ];
        schemas
    }
}

#[async_trait::async_trait]
impl Agent for ReActAgent {
    fn name(&self) -> &str { &self.name }
    fn llm(&self) -> &HelloAgentsLLM { &self.llm }
    fn system_prompt(&self) -> Option<&str> { self.system_prompt.as_deref() }
    fn config(&self) -> Option<&Config> {
        self.config.as_ref()
    }

    async fn run(&mut self, input_text: &str) -> Result<String, HelloAgentsError> {
        let mut messages = Vec::new();

        if let Some(ref sys) = self.system_prompt {
            messages.push(Message::new(sys.clone(), MessageRole::System));
        }
        messages.push(Message::new(input_text, MessageRole::User));

        let tool_schemas = self.build_tool_schemas();

        for step in 0..self.max_steps {
            println!("\n--- 第 {} 步 ---", step + 1);

            let response = self.llm.invoke_with_tools(
                &messages,
                &tool_schemas,
                &ToolChoice::Auto,
                None,
            ).await?;

            if response.tool_calls.is_empty() {
                let answer = response.content.unwrap_or_else(|| "无法回答".into());
                self.history.push(Message::new(input_text, MessageRole::User));
                self.history.push(Message::new(answer.clone(), MessageRole::Assistant));
                return Ok(answer);
            }

            // 处理工具调用
            for tc in &response.tool_calls {
                if tc.name == "Finish" {
                    let args: HashMap<String, serde_json::Value> =
                        serde_json::from_str(&tc.arguments).unwrap_or_default();
                    let answer = args.get("answer").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    println!("🎉 最终答案: {}", answer);
                    self.history.push(Message::new(input_text, MessageRole::User));
                    self.history.push(Message::new(answer.clone(), MessageRole::Assistant));
                    return Ok(answer);
                }

                if tc.name == "Thought" {
                    let args: HashMap<String, serde_json::Value> =
                        serde_json::from_str(&tc.arguments).unwrap_or_default();
                    let reasoning = args.get("reasoning").and_then(|v| v.as_str()).unwrap_or("");
                    println!("💭 思考: {}", reasoning);

                    // 添加 Thought 结果
                    let mut tool_msg = serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "content": format!("已记录推理过程: {}", reasoning),
                    });
                    messages.push(Message::new(
                        serde_json::to_string(&tool_msg).unwrap(),
                        MessageRole::Tool,
                    ));
                }
            }
        }

        let answer = "已达到最大步数限制".to_string();
        self.history.push(Message::new(input_text, MessageRole::User));
        self.history.push(Message::new(answer.clone(), MessageRole::Assistant));
        Ok(answer)
    }

    async fn run_stream(
        &mut self,
        _input_text: &str,
    ) -> Result<mpsc::UnboundedReceiver<StreamEvent>, HelloAgentsError> {
        Err(HelloAgentsError::Agent("ReActAgent run_stream 未实现".into()))
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