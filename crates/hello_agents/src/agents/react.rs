use crate::core::agent_runtime::AgentRuntime;
use crate::core::traits::agent::Agent;
use crate::core::traits::tool::Tool;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::message::{Message, ToolCall, ToolDefinition};
use crate::core::types::response::ToolResponse;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;

const REACT_PROMPT: &str = "你是一个具备推理和行动能力的 AI 助手。

## 工作流程
你可以通过调用工具来完成任务：

1. **Thought 工具**：用于记录你的推理过程和分析
   - 在需要思考时调用
   - 参数：reasoning（你的推理内容）

2. **业务工具**：用于获取信息或执行操作
   - 根据任务需求选择合适的工具
   - 可以多次调用不同工具

3. **Finish 工具**：用于返回最终答案
   - 当你有足够信息得出结论时调用
   - 参数：answer（最终答案）

## 重要提醒
- 主动使用 Thought 工具记录推理过程
- 可以多次调用工具获取信息
- 只有在确信有足够信息时才调用 Finish";

pub struct ThoughtTool;
#[async_trait]
impl Tool for ThoughtTool {
    fn name(&self) -> &str {
        "Thought"
    }
    fn description(&self) -> &str {
        "记录推理过程"
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{"reasoning":{"type":"string","description":"推理内容"}},"required":["reasoning"]})
    }
    async fn execute(&self, _args: Value) -> Result<ToolResponse, HelloAgentError> {
        Ok(ToolResponse::success("推理已记录"))
    }
}

pub struct FinishTool;
#[async_trait]
impl Tool for FinishTool {
    fn name(&self) -> &str {
        "Finish"
    }
    fn description(&self) -> &str {
        "返回最终答案"
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{"answer":{"type":"string","description":"最终答案"}},"required":["answer"]})
    }
    async fn execute(&self, _args: Value) -> Result<ToolResponse, HelloAgentError> {
        Ok(ToolResponse::success("任务完成"))
    }
}

pub struct ReActAgent {
    name: String,
    system_prompt: Option<String>,
    max_steps: usize,
    pub steps: usize,  // 公开统计
    pub tokens: usize, // 公开统计
}

impl ReActAgent {
    pub fn new(name: &str, system_prompt: Option<String>, max_steps: usize) -> Self {
        Self {
            name: name.into(),
            system_prompt,
            max_steps,
            steps: 0,
            tokens: 0,
        }
    }
}

#[async_trait]
impl Agent for ReActAgent {
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
        } else {
            messages.push(Message::system(REACT_PROMPT));
        }
        {
            let hist = runtime.history.lock().unwrap();
            messages.extend(hist.messages());
        }
        messages.push(Message::user(input));

        let mut tool_defs = vec![
            ToolDefinition {
                def_type: "function",
                function: json!({
                    "name": "Thought",
                    "description": ThoughtTool.description(),
                    "parameters": ThoughtTool.parameters()
                }),
            },
            ToolDefinition {
                def_type: "function",
                function: json!({
                    "name": "Finish",
                    "description": FinishTool.description(),
                    "parameters": FinishTool.parameters()
                }),
            },
        ];
        for name in runtime.tools.list_tools() {
            if let Some(t) = runtime.tools.get_tool(&name) {
                tool_defs.push(ToolDefinition {
                    def_type: "function",
                    function: json!({
                        "name": t.name(),
                        "description": t.description(),
                        "parameters": t.parameters()
                    }),
                });
            }
        }

        while self.steps < self.max_steps {
            self.steps += 1;
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
                    let args: Value = serde_json::from_str(&tc.function.arguments)?;
                    let result = if tc.function.name == "Thought" {
                        ThoughtTool.execute(args).await?
                    } else if tc.function.name == "Finish" {
                        let answer = args["answer"].as_str().unwrap_or("").to_string();
                        runtime
                            .history
                            .lock()
                            .unwrap()
                            .add_message(Message::user(input));
                        runtime
                            .history
                            .lock()
                            .unwrap()
                            .add_message(Message::assistant(&answer));
                        return Ok(answer);
                    } else {
                        match runtime.tools.execute(&tc.function.name, args).await {
                            Ok(resp) => resp,
                            Err(e) => ToolResponse::error(
                                "EXECUTION_ERROR",
                                &format!("工具执行失败: {}", e),
                            ),
                        }
                    };
                    messages.push(Message::tool(tc.id.clone(), &result.text));
                }
            }
        }
        Err(HelloAgentError::Agent("max steps exceeded".into()))
    }
}
