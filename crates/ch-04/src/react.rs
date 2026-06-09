use std::sync::Arc;
use crate::llm_client::{LLMClient};
use crate::client_message::{Message};
use crate::tools::ToolExecutor;
use regex::Regex;

const REACT_PROMPT_TEMPLATE: &str = r#"
请注意，你是一个有能力调用外部工具的智能助手。

可用工具如下：
{tools}

请严格按照以下格式进行回应：

Thought: 你的思考过程，用于分析问题、拆解任务和规划下一步行动。
Action: 你决定采取的行动，必须是以下格式之一：
- `{tool_name}[{tool_input}]`：调用一个可用工具。
- `Finish[最终答案]`：当你认为已经获得最终答案时。
- 当你收集到足够的信息，能够回答用户的最终问题时，你必须在`Action:`字段后使用 `Finish[最终答案]` 来输出最终答案。

现在，请开始解决以下问题：
Question: {question}
History: {history}
"#;

pub struct ReActAgent {
    llm_client: Arc<LLMClient>,
    tool_executor: ToolExecutor,
    max_steps: usize,
}

impl ReActAgent {
    pub fn new(llm_client: Arc<LLMClient>, tool_executor: ToolExecutor, max_steps: usize) -> Self {
        ReActAgent {
            llm_client,
            tool_executor,
            max_steps,
        }
    }

    pub async fn run(&mut self, question: &str) -> Option<String> {
        let mut history: Vec<String> = Vec::new();
        let mut current_step = 0;

        while current_step < self.max_steps {
            current_step += 1;
            println!("\n--- 第 {} 步 ---", current_step);

            let tools_desc = self.tool_executor.get_available_tools();
            let history_str = history.join("\n");
            let prompt = REACT_PROMPT_TEMPLATE
                .replace("{tools}", &tools_desc)
                .replace("{question}", question)
                .replace("{history}", &history_str);

            let messages = vec![Message {
                role: "user".to_string(),
                content: prompt,
            }];

            let response_text = match self.llm_client.think(messages, 0.0).await {
                Ok(text) => text,
                Err(e) => {
                    println!("错误：LLM未能返回有效响应。{e}");
                    break;
                }
            };

            let (thought, action) = Self::parse_output(&response_text);
            if let Some(t) = &thought {
                println!("🤔 思考: {}", t);
            }
            if action.is_none() {
                println!("警告：未能解析出有效的Action，流程终止。");
                break;
            }

            let action_text = action.unwrap();
            if action_text.starts_with("Finish") {
                let final_answer = Self::parse_action_input(&action_text);
                println!("🎉 最终答案: {}", final_answer);
                return Some(final_answer);
            }

            let (tool_name, tool_input) = match Self::parse_action(&action_text) {
                Some((name, input)) => (name, input),
                None => {
                    history.push("Observation: 无效的Action格式，请检查。".to_string());
                    continue;
                }
            };

            println!("🎬 行动: {}[{}]", tool_name, tool_input);
            let observation = match self.tool_executor.get_tool(&tool_name) {
                Some(func) => (*func)(tool_input.clone()).await,  // ← 解引用后调用
                None => format!("错误：未找到名为 '{}' 的工具。", tool_name),
            };

            println!("👀 观察: {}", observation);
            history.push(format!("Action: {}", action_text));
            history.push(format!("Observation: {}", observation));
        }
        println!("已达到最大步数，流程终止。");
        None
    }

    fn parse_output(text: &str) -> (Option<String>, Option<String>) {
        // let thought_re = Regex::new(r"Thought:\s*((?s:.*?))(?=\nAction:|$)").unwrap();
        let thought_re = Regex::new(r"(?s)Thought:\s*(.*?)(?:\nAction:|$)").unwrap();
        let thought = thought_re
            .captures(text)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().trim().to_string());

        let action_re = Regex::new(r"Action:\s*((?s:.*?))$").unwrap();
        let action = action_re
            .captures(text)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().trim().to_string());

        (thought, action)
    }

    fn parse_action(action_text: &str) -> Option<(String, String)> {
        let re = Regex::new(r"(\w+)\[(.*)\]").unwrap();
        re.captures(action_text).map(|cap| {
            (cap[1].to_string(), cap[2].to_string())
        })
    }

    fn parse_action_input(action_text: &str) -> String {
        let re = Regex::new(r"\w+\[(.*)\]").unwrap();
        re.captures(action_text)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    }
}