// agents/simple.rs
// 简单的对话 Agent，支持可选的工具调用
// 包含完整的工具定义修复、统计信息收集

use crate::core::agent_runtime::AgentRuntime;
use crate::core::traits::agent::Agent;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::message::{Message, ToolDefinition};
use async_trait::async_trait;

pub struct SimpleAgent {
    name: String,
    system_prompt: Option<String>,
    max_tool_rounds: usize,
    /// 统计：已执行的步骤数
    pub steps: usize,
    /// 统计：消耗的总 Token
    pub tokens: usize,
}

impl SimpleAgent {
    pub fn new(name: &str, system_prompt: Option<String>, max_tool_rounds: usize) -> Self {
        Self {
            name: name.into(),
            system_prompt,
            max_tool_rounds,
            steps: 0,
            tokens: 0,
        }
    }
}

#[async_trait]
impl Agent for SimpleAgent {
    fn name(&self) -> &str {
        &self.name
    }

    async fn run(
        &mut self,
        input: &str,
        runtime: &AgentRuntime,
    ) -> Result<String, HelloAgentError> {
        self.steps = 0;
        self.tokens = 0;

        let mut messages: Vec<Message> = vec![];
        if let Some(sys) = &self.system_prompt {
            messages.push(Message::system(sys));
        }
        {
            let hist = runtime.history.lock().unwrap();
            messages.extend(hist.messages());
        }
        messages.push(Message::user(input));

        for _ in 0..self.max_tool_rounds {
            self.steps += 1;

            let tools = runtime.tools.list_tools();
            let mut tool_defs = Vec::new();
            for name in &tools {
                if let Some(t) = runtime.tools.get_tool(name) {
                    // 构建完整的 function 定义，包含 name 和 description
                    tool_defs.push(ToolDefinition {
                        def_type: "function",
                        function: serde_json::json!({
                            "name": t.name(),
                            "description": t.description(),
                            "parameters": t.parameters()
                        }),
                    });
                }
            }

            let resp = runtime
                .llm
                .chat(&messages, Some(&tool_defs), Some("auto"))
                .await?;
            self.tokens += resp.usage.total_tokens as usize;

            if resp.tool_calls.is_empty() {
                let reply = resp.content.unwrap_or_default();
                runtime
                    .history
                    .lock()
                    .unwrap()
                    .add_message(Message::user(input));
                runtime
                    .history
                    .lock()
                    .unwrap()
                    .add_message(Message::assistant(&reply));
                return Ok(reply);
            } else {
                messages.push(Message::assistant_tool_calls(
                    resp.content.clone(),
                    resp.tool_calls.clone(),
                ));
                for tc in &resp.tool_calls {
                    let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)?;
                    let result = runtime.tools.execute(&tc.function.name, args).await?;
                    messages.push(Message::tool(tc.id.clone(), &result.text));
                }
            }
        }
        Err(HelloAgentError::Agent("max tool rounds exceeded".into()))
    }
}
