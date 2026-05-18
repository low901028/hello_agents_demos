use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

use super::lang_graph::{GraphState, InMemorySaver, Message, StateData, StateGraphBuilder};
use crate::simple_agent::simple_agent_client::HelloAgentsLLM;
use super::tavily_search::TavilyClient;

#[derive(Debug, Clone)]
pub struct SearchState {
    pub user_query: String,
    pub search_query: String,
    pub search_results: String,
    pub final_answer: String,
    pub step: String,
}

impl StateData for SearchState {}

pub struct SearchAssistant {
    llm: Arc<HelloAgentsLLM>,
    tavily: Arc<TavilyClient>,
}

impl SearchAssistant {
    pub fn new(llm: Arc<HelloAgentsLLM>, tavily: Arc<TavilyClient>) -> Self {
        Self { llm, tavily }
    }

    /// 构建并编译图（等价 Python 的 create_search_assistant）
    pub fn compile(&self) -> super::lang_graph::CompiledGraph<SearchState> {
        let llm = self.llm.clone();
        let tavily = self.tavily.clone();
        let mut builder = StateGraphBuilder::<SearchState>::new();

        // ========== 节点 1：理解查询 ==========
        let llm1 = llm.clone();
        builder.add_node("understand", move |state: GraphState<SearchState>| {
            let llm = llm1.clone();
            async move {
                let user_message = state.messages.iter().rev()
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

                let response = llm.think(
                    vec![crate::simple_agent::simple_agent_client::Message { role: "system".into(), content: prompt, name: None }],
                    0.7,
                    None
                ).await.unwrap_or_default();

                let search_query = ["搜索词：", "搜索关键词："].iter()
                    .find_map(|kw| response.find(kw).map(|pos| {
                        response[pos + kw.len()..].lines().next().unwrap_or("").trim().to_string()
                    }))
                    .unwrap_or(user_message);

                let mut new_state = state;
                new_state.data.user_query = response.clone();
                new_state.data.search_query = search_query;
                new_state.data.step = "understood".into();
                new_state.messages.push(Message::new("assistant", format!("我理解您的需求：{}", response), Some("助手".into())));
                Ok(new_state)
            }
        });

        // ========== 节点 2：搜索 ==========
        let tavily1 = tavily.clone();
        builder.add_node("search", move |state: GraphState<SearchState>| {
            let tavily = tavily1.clone();
            async move {
                let search_query = state.data.search_query.clone();
                let mut new_state = state;

                match tavily.search(&search_query, 5).await {
                    Ok(response) => {
                        new_state.data.search_results = TavilyClient::format_results(&response);
                        new_state.data.step = "searched".into();
                    }
                    Err(e) => {
                        new_state.data.search_results = format!("搜索失败：{}", e);
                        new_state.data.step = "search_failed".into();
                    }
                }
                new_state.messages.push(Message::new("assistant", "✅ 搜索完成！找到了相关信息，正在为您整理答案...", Some("助手".into())));
                Ok(new_state)
            }
        });

        // ========== 节点 3：生成答案 ==========
        let llm2 = llm.clone();
        builder.add_node("answer", move |state: GraphState<SearchState>| {
            let llm = llm2.clone();
            async move {
                let prompt = if state.data.step == "search_failed" {
                    format!("搜索API暂时不可用，请基于您的知识回答用户的问题：\n\n用户问题：{}\n\n请提供一个有用的回答，并说明这是基于已有知识的回答。", state.data.user_query)
                } else {
                    format!(
                        r#"基于以下搜索结果为用户提供完整、准确的答案：

用户问题：{}
搜索结果：{}

请要求：
1. 综合搜索结果，提供准确、有用的回答
2. 如果是技术问题，提供具体的解决方案或代码
3. 引用重要信息的来源
4. 回答要结构清晰、易于理解
5. 如果搜索结果不够完整，请说明并提供补充建议"#,
                        state.data.user_query, state.data.search_results
                    )
                };

                let response = llm.think(
                    vec![crate::simple_agent::simple_agent_client::Message { role: "system".into(), content: prompt, name: None }],
                    0.7,
                    None
                ).await.unwrap_or_else(|_| "抱歉，生成答案时出错。".into());

                let mut new_state = state;
                new_state.data.final_answer = response.clone();
                new_state.data.step = "completed".into();
                new_state.messages.push(Message::new("assistant", response, Some("助手".into())));
                Ok(new_state)
            }
        });

        // ========== 边（等价 Python 的 add_edge(START, "understand") 等） ==========
        builder.set_entry_point("understand");
        builder.add_edge("understand", "search");
        builder.add_edge("search", "answer");
        // answer 无出边 = END

        let checkpointer = InMemorySaver::new();
        builder.compile(checkpointer)
    }
}