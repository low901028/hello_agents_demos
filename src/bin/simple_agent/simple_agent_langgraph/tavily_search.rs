use anyhow::Result;
use std::sync::Arc;

use crate::simple_agent::simple_agent_client::{HelloAgentsLLM, Message};
use crate::simple_agent::simple_agent_langgraph::search_agent::TavilyClient;

/// 搜索状态，对应 Python 的 SearchState
#[derive(Debug, Clone)]
pub struct SearchState {
    pub messages: Vec<Message>,
    pub user_query: String,
    pub search_query: String,
    pub search_results: String,
    pub final_answer: String,
    pub step: String,
}

/// 中间状态事件（用于流式输出）
#[derive(Debug, Clone)]
pub enum AgentEvent {
    Stage(String, String),  // (stage_name, message)
    Final(String),          // 最终答案
}

pub struct SearchAgent {
    llm: Arc<HelloAgentsLLM>,
    tavily: Arc<TavilyClient>,
}

impl SearchAgent {
    pub fn new(llm: Arc<HelloAgentsLLM>, tavily: Arc<TavilyClient>) -> Self {
        Self { llm, tavily }
    }

    /// 执行完整搜索流程，通过 channel 发送中间状态
    pub async fn run(
        &self,
        user_message: String,
        tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
    ) -> Result<String> {
        let mut state = SearchState {
            messages: vec![Message {
                role: "user".into(),
                content: user_message.clone(),
                name: Some("用户".into()),
            }],
            user_query: String::new(),
            search_query: String::new(),
            search_results: String::new(),
            final_answer: String::new(),
            step: "start".into(),
        };

        // 步骤 1：理解查询
        state = self.understand_query(state).await?;
        let _ = tx.send(AgentEvent::Stage("understand".into(), format!(
            "我理解您的需求：{}", state.user_query
        )));

        // 步骤 2：搜索
        state = self.tavily_search(state).await;
        let _ = tx.send(AgentEvent::Stage("search".into(),
                                          "✅ 搜索完成！找到了相关信息，正在为您整理答案...".into()
        ));

        // 步骤 3：生成答案
        state = self.generate_answer(state).await?;
        let _ = tx.send(AgentEvent::Final(state.final_answer.clone()));

        Ok(state.final_answer)
    }

    /// 步骤 1：理解用户查询并提取搜索关键词
    async fn understand_query(&self, state: SearchState) -> Result<SearchState> {
        let user_message = state.messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.clone())
            .unwrap_or_default();

        let prompt = format!(
            r#"分析用户的查询："{}"

请完成两个任务：
1. 简洁总结用户想要了解什么
2. 生成最适合搜索的关键词（中英文均可，要精准）

格式：
理解：[用户需求总结]
搜索词：[最佳搜索关键词]"#,
            user_message
        );

        let response = self.llm.think(
            vec![Message {
                role: "system".into(),
                content: prompt,
                name: None,
            }],
            0.7,
            None
        ).await?;

        // 提取搜索关键词
        let search_query = if let Some(pos) = response.find("搜索词：") {
            response[pos + "搜索词：".len()..]
                .lines()
                .next()
                .unwrap_or(&user_message)
                .trim()
                .to_string()
        } else if let Some(pos) = response.find("搜索关键词：") {
            response[pos + "搜索关键词：".len()..]
                .lines()
                .next()
                .unwrap_or(&user_message)
                .trim()
                .to_string()
        } else {
            user_message
        };

        let mut new_state = state;
        new_state.user_query = response;
        new_state.search_query = search_query;
        new_state.step = "understood".into();
        new_state.messages.push(Message {
            role: "assistant".into(),
            content: format!("我理解您的需求：{}", new_state.user_query),
            name: Some("助手".into()),
        });

        Ok(new_state)
    }

    /// 步骤 2：Tavily 搜索
    async fn tavily_search(&self, state: SearchState) -> SearchState {
        let mut new_state = state;

        match self.tavily.search(&new_state.search_query, 5).await {
            Ok(response) => {
                new_state.search_results = TavilyClient::format_results(&response);
                new_state.step = "searched".into();
            }
            Err(e) => {
                let error_msg = format!("搜索失败：{}", e);
                eprintln!("❌ {}", error_msg);
                new_state.search_results = error_msg;
                new_state.step = "search_failed".into();
            }
        }

        new_state.messages.push(Message {
            role: "assistant".into(),
            content: "✅ 搜索完成！正在整理答案...".into(),
            name: Some("助手".into()),
        });

        new_state
    }

    /// 步骤 3：生成最终答案
    async fn generate_answer(&self, state: SearchState) -> Result<SearchState> {
        let prompt = if state.step == "search_failed" {
            format!(
                r#"搜索API暂时不可用，请基于您的知识回答用户的问题：

用户问题：{}

请提供一个有用的回答，并说明这是基于已有知识的回答。"#,
                state.user_query
            )
        } else {
            format!(
                r#"基于以下搜索结果为用户提供完整、准确的答案：

用户问题：{}

搜索结果：
{}

请要求：
1. 综合搜索结果，提供准确、有用的回答
2. 如果是技术问题，提供具体的解决方案或代码
3. 引用重要信息的来源
4. 回答要结构清晰、易于理解
5. 如果搜索结果不够完整，请说明并提供补充建议"#,
                state.user_query, state.search_results
            )
        };

        let response = self.llm.think(
            vec![Message {
                role: "system".into(),
                content: prompt,
                name: None,
            }],
            0.7,
            None
        ).await?;

        let mut new_state = state;
        new_state.final_answer = response.clone();
        new_state.step = "completed".into();
        new_state.messages.push(Message {
            role: "assistant".into(),
            content: response,
            name: Some("助手".into()),
        });

        Ok(new_state)
    }
}