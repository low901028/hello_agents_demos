// core/agent_runtime.rs
use crate::core::observability::TraceLogger;
use crate::core::traits::history::HistoryManager;
use crate::core::traits::llm_provider::LlmProvider;
use crate::core::traits::session_store::SessionStore;
use crate::core::traits::tool_registry::ToolRegistry;
use crate::core::types::config::Config;
use std::sync::{Arc, Mutex};
use crate::core::traits::skill_optimizer::SkillOptimizer;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::session::SessionData;

pub struct AgentRuntime {
    pub llm: Arc<dyn LlmProvider>,
    pub tools: Arc<dyn ToolRegistry>,
    pub history: Arc<Mutex<Box<dyn HistoryManager>>>,
    pub config: Config,
    pub trace_logger: Option<Arc<dyn TraceLogger>>,
    pub session_store: Option<Arc<dyn SessionStore>>,
    pub skill_optimizer: Option<Arc<dyn SkillOptimizer>>,
}

impl AgentRuntime {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        tools: Arc<dyn ToolRegistry>,
        history: Arc<Mutex<Box<dyn HistoryManager>>>,
        config: Config,
    ) -> Self {
        Self {
            llm,
            tools,
            history,
            config,
            trace_logger: None,
            session_store: None,
            skill_optimizer: None,
        }
    }

    pub fn with_trace_logger(mut self, logger: Arc<dyn TraceLogger>) -> Self {
        self.trace_logger = Some(logger);
        self
    }

    pub fn with_session_store(mut self, store: Arc<dyn SessionStore>) -> Self {
        self.session_store = Some(store);
        self
    }

    pub async fn save_session(&self, session: &SessionData) -> Result<Option<String>, HelloAgentError> {
        if let Some(store) = &self.session_store {
            Ok(Some(store.save(session).await?))
        } else {
            Ok(None)
        }
    }

    pub fn with_skill_optimizer(mut self, optimizer: Arc<dyn SkillOptimizer>) -> Self {
        self.skill_optimizer = Some(optimizer);
        self
    }
}
