use anyhow::Result;
use regex::Regex;
use crate::simple_agent::simple_agent_client::{HelloAgentsLLM, Message};
use crate::simple_agent::simple_agent_utils_register::ToolExecutor;

pub const REACT_PROMPT_TEMPLATE: &str = r#"
请注意，你是一个有能力调用外部工具的智能助手。

可用工具如下：
{tools}

请严格按照以下格式进行回应：

Thought: 你的思考过程，用于分析问题、拆解任务和规划下一步行动。
Action: 你决定采取的行动，必须是以下格式之一：
- `{{tool_name}}[{{tool_input}}]`：调用一个可用工具。
- `Finish[最终答案]`：当你认为已经获得最终答案时。
- 当你收集到足够的信息，能够回答用户的最终问题时，你必须在`Action:`字段后使用 `Finish[最终答案]` 来输出最终答案。


现在，请开始解决以下问题：
Question: {question}
History: {history}
"#;

pub struct ReActAgent {
    llm_client: HelloAgentsLLM,
    tool_executor: ToolExecutor,
    max_steps: usize,
    history: Vec<String>,
}

impl ReActAgent {
    pub fn new(llm_client: HelloAgentsLLM, tool_executor: ToolExecutor, max_steps: usize) -> Self {
        Self {
            llm_client,
            tool_executor,
            max_steps,
            history: Vec::new(),
        }
    }

    /// 运行 ReAct 循环，返回最终答案或 None（超过最大步数）
    pub async fn run(&mut self, question: &str) -> Option<String> {
        self.history.clear();
        let mut current_step = 0;

        while current_step < self.max_steps {
            current_step += 1;
            println!("\n--- 第 {} 步 ---", current_step);

            let tools_desc = self.tool_executor.available_tools();
            let history_str = self.history.join("\n");
            let prompt = REACT_PROMPT_TEMPLATE
                .replace("{tools}", &tools_desc)
                .replace("{question}", question)
                .replace("{history}", &history_str);

            let messages = vec![
                Message {
                    role: "user".into(),
                    content: prompt,
                },
            ];

            // 调用 LLM（异步）
            let response_text = match self.llm_client.think(messages, 0.0).await {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("错误：LLM 返回错误 - {}", e);
                    break;
                }
            };

            let (thought, action) = Self::parse_output(&response_text);
            if let Some(ref th) = thought {
                println!("🤔 思考: {}", th);
            }
            let action = match action {
                Some(a) => a,
                None => {
                    println!("警告：未能解析出有效的Action，流程终止。");
                    break;
                }
            };

            if action.starts_with("Finish") {
                let final_answer = Self::parse_action_input(&action).unwrap_or_default();
                println!("🎉 最终答案: {}", final_answer);
                return Some(final_answer);
            }

            // 解析工具名称和输入
            let (tool_name, tool_input) = match Self::parse_action(&action) {
                Some(pair) => pair,
                None => {
                    self.history.push("Observation: 无效的Action格式，请检查。".to_string());
                    continue;
                }
            };

            println!("🎬 行动: {}[{}]", tool_name, tool_input);

            let observation = match self.tool_executor.execute(&tool_name, &tool_input).await {
                Ok(obs) => obs,
                Err(e) => format!("错误：{}", e),
            };

            println!("👀 观察: {}", observation);
            self.history.push(format!("Action: {}", action));
            self.history.push(format!("Observation: {}", observation));
        }

        println!("已达到最大步数，流程终止。");
        None
    }

    /// 从 LLM 输出中解析 Thought 和 Action
    fn parse_output(text: &str) -> (Option<String>, Option<String>) {
        // 提取 Thought: 内容，停在 Action: 前或文本末尾
        let thought_re = Regex::new(r"(?s)Thought:\s*(.*?)(?:\nAction:|$)").unwrap();
        let thought = thought_re
            .captures(text)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string());

        // 提取 Action: 直到文本末尾
        let action_re = Regex::new(r"(?s)Action:\s*(.*)").unwrap();
        let action = action_re
            .captures(text)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string());

        (thought, action)
    }

    /// 解析 Action 格式：ToolName[Input]
    fn parse_action(action_text: &str) -> Option<(String, String)> {
        let re = Regex::new(r"(?s)^(\w+)\[(.*)\]$").unwrap();
        re.captures(action_text).map(|caps| {
            (
                caps.get(1).unwrap().as_str().to_string(),
                caps.get(2).unwrap().as_str().to_string(),
            )
        })
    }

    /// 解析 Finish[答案] 中的答案部分
    fn parse_action_input(action_text: &str) -> Option<String> {
        let re = Regex::new(r"(?s)^\w+\[(.*)\]$").unwrap();
        re.captures(action_text)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_output() {
        let text = "Thought: 我需要查询天气\nAction: Search[北京天气]";
        let (thought, action) = ReActAgent::parse_output(text);
        assert_eq!(thought, Some("我需要查询天气".to_string()));
        assert_eq!(action, Some("Search[北京天气]".to_string()));
    }

    #[test]
    fn test_parse_output_finish() {
        let text = "Thought: 已经得到答案\nAction: Finish[答案是42]";
        let (thought, action) = ReActAgent::parse_output(text);
        assert_eq!(thought, Some("已经得到答案".to_string()));
        assert_eq!(action, Some("Finish[答案是42]".to_string()));
    }

    #[test]
    fn test_parse_action() {
        let (name, input) = ReActAgent::parse_action("Search[华为最新手机]").unwrap();
        assert_eq!(name, "Search");
        assert_eq!(input, "华为最新手机");
    }

    #[test]
    fn test_parse_action_input() {
        let input = ReActAgent::parse_action_input("Finish[最终结果]").unwrap();
        assert_eq!(input, "最终结果");
    }

    #[test]
    fn test_parse_output_multiline_thought() {
        let text = "Thought: 这是第一行\n这是第二行\nAction: Tool[数据]";
        let (thought, _) = ReActAgent::parse_output(text);
        assert_eq!(thought, Some("这是第一行\n这是第二行".to_string()));
    }
}