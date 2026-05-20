use crate::hello_agent::context::history::HistoryManager;
use crate::hello_agent::context::token_counter::TokenCounter;
use crate::hello_agent::context::truncator::ObservationTruncator;
use crate::hello_agent::core::config::Config;
use crate::hello_agent::core::llm::HelloAgentsLlm;
use crate::hello_agent::core::message::{Message, MessageRole};
use crate::hello_agent::core::session_store::SessionStore;
use crate::hello_agent::core::streaming::{StreamEvent, StreamEventType};
use crate::hello_agent::observability::trace_logger::TraceLogger;
use crate::hello_agent::skills::loader::SkillLoader;
use crate::hello_agent::tools::registry::ToolRegistry;
use crate::hello_agent::tools::tool_filter::ToolFilter;
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    fn run(&self, input_text: &str) -> String;
    fn get_system_prompt(&self) -> Option<&str> {
        None
    }
    fn max_steps_mut(&mut self) -> Option<&mut usize> {
        None
    }
    fn max_steps(&self) -> Option<usize> {
        None
    }
    fn add_message(&mut self, message: Message);
    fn get_history(&self) -> Vec<Message>;
    fn clear_history(&mut self);

    async fn arun_stream(&self, input_text: &str) -> mpsc::Receiver<StreamEvent> {
        let (tx, rx) = mpsc::channel(100);
        let name = self.name().to_string();
        let input = input_text.to_string();
        tokio::spawn(async move {
            let _ = tx
                .send(StreamEvent::with_data(
                    StreamEventType::AgentStart,
                    &name,
                    "input_text",
                    input,
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

    fn run_as_subagent(
        &self,
        task: &str,
        _tool_filter: Option<&dyn ToolFilter>,
        _return_summary: bool,
        _max_steps_override: Option<usize>,
    ) -> HashMap<String, serde_json::Value> {
        let result = self.run(task);
        let success = !result.contains("无法在限定步数内完成") && !result.starts_with("错误:");
        let mut output = HashMap::with_capacity(4);
        output.insert("success".into(), serde_json::json!(success));
        output.insert("summary".into(), serde_json::json!(result.clone()));
        output.insert("result".into(), serde_json::json!(result));
        output.insert(
            "metadata".into(),
            serde_json::json!({"steps":0,"duration_seconds":0.0,"tools_used":[]}),
        );
        output
    }
}

pub struct AgentBase {
    pub name: String,
    pub llm: HelloAgentsLlm,
    pub system_prompt: Option<String>,
    pub config: Config,
    pub tool_registry: Option<Arc<ToolRegistry>>,
    pub history_manager: RwLock<HistoryManager>,
    pub truncator: ObservationTruncator,
    pub token_counter: RwLock<TokenCounter>,
    history_token_count: RwLock<usize>,
    pub trace_logger: Option<RwLock<TraceLogger>>,
    pub skill_loader: Option<SkillLoader>,
    pub session_store: Option<SessionStore>,
    session_metadata: RwLock<HashMap<String, serde_json::Value>>,
    start_time: chrono::DateTime<chrono::Utc>,
}

pub fn create_agent_base(
    name: &str,
    llm: HelloAgentsLlm,
    system_prompt: Option<String>,
    config: Option<Config>,
    tool_registry: Option<Arc<ToolRegistry>>,
) -> AgentBase {
    AgentBase::new(name, llm, system_prompt, config, tool_registry)
}

impl AgentBase {
    pub fn new(
        name: &str,
        llm: HelloAgentsLlm,
        system_prompt: Option<String>,
        config: Option<Config>,
        tool_registry: Option<Arc<ToolRegistry>>,
    ) -> Self {
        let config = config.unwrap_or_default();
        let history_manager =
            HistoryManager::new(config.min_retain_rounds, config.compression_threshold);
        let truncator = ObservationTruncator::new(
            config.tool_output_max_lines,
            config.tool_output_max_bytes,
            &config.tool_output_truncate_direction,
            &config.tool_output_dir,
        );
        let token_counter = TokenCounter::new(&llm.model);
        let trace_logger = if config.trace_enabled {
            TraceLogger::new(
                &config.trace_dir,
                config.trace_sanitize,
                config.trace_html_include_raw_response,
            )
            .ok()
            .map(RwLock::new)
        } else {
            None
        };
        let skill_loader = if config.skills_enabled {
            let skills_path = Path::new(&config.skills_dir);
            let mut loader = SkillLoader::new(skills_path.to_path_buf());
            if config.skills_auto_register {
                if let Some(ref reg) = tool_registry {
                    reg.register_tool(
                        Arc::new(
                            crate::hello_agent::tools::builtin::skill_tool::SkillTool::new(
                                loader.clone(),
                            ),
                        ),
                        false,
                    );
                }
            }
            Some(loader)
        } else {
            None
        };
        let session_store = if config.session_enabled {
            SessionStore::new(&config.session_dir).ok()
        } else {
            None
        };
        let now = chrono::Utc::now();
        let mut metadata = HashMap::with_capacity(4);
        metadata.insert("created_at".into(), serde_json::json!(now.to_rfc3339()));
        metadata.insert("total_tokens".into(), serde_json::json!(0));
        metadata.insert("total_steps".into(), serde_json::json!(0));
        metadata.insert("duration_seconds".into(), serde_json::json!(0));
        let mut base = AgentBase {
            name: name.to_string(),
            llm,
            system_prompt,
            config: config.clone(),
            tool_registry: tool_registry.clone(),
            history_manager: RwLock::new(history_manager),
            truncator,
            token_counter: RwLock::new(token_counter),
            history_token_count: RwLock::new(0),
            trace_logger,
            skill_loader,
            session_store,
            session_metadata: RwLock::new(metadata),
            start_time: now,
        };
        if config.subagent_enabled {
            if tool_registry.is_some() {
                base.register_task_tool();
            }
        }
        if config.todowrite_enabled {
            if tool_registry.is_some() {
                base.register_todowrite_tool();
            }
        }
        if config.devlog_enabled {
            if tool_registry.is_some() {
                base.register_devlog_tool();
            }
        }
        base
    }

    pub fn add_message(&self, message: Message) {
        let new_tokens = {
            let mut tc = self.token_counter.write();
            tc.count_message(&message)
        };
        {
            self.history_manager.write().append(message);
        }
        {
            *self.history_token_count.write() += new_tokens;
        }
        let should_compress = {
            *self.history_token_count.read()
                > (self.config.context_window as f64 * self.config.compression_threshold) as usize
        };
        if should_compress {
            self.compress_history();
        }
    }

    pub fn get_history(&self) -> Vec<Message> {
        self.history_manager.read().get_history()
    }

    pub fn clear_history(&self) {
        self.history_manager.write().clear();
        *self.history_token_count.write() = 0;
        self.token_counter.write().clear_cache();
    }

    fn compress_history(&self) {
        let history = self.history_manager.read().get_history();
        let summary = if self.config.enable_smart_compression {
            self.generate_smart_summary(&history)
        } else {
            self.generate_simple_summary(&history)
        };
        {
            self.history_manager.write().compress(&summary);
        }
        let new_history = self.history_manager.read().get_history();
        let new_count = { self.token_counter.write().count_messages(&new_history) };
        *self.history_token_count.write() = new_count;
    }

    fn generate_simple_summary(&self, history: &[Message]) -> String {
        let rounds = self.history_manager.read().estimate_rounds();
        let users = history
            .iter()
            .filter(|m| m.role == MessageRole::User)
            .count();
        let assistants = history
            .iter()
            .filter(|m| m.role == MessageRole::Assistant)
            .count();
        format!(
            "会话包含{}轮：{}条用户消息，{}条助手消息，共{}条\n(已压缩，保留最近{}轮)",
            rounds,
            users,
            assistants,
            history.len(),
            self.config.min_retain_rounds
        )
    }

    fn generate_smart_summary(&self, history: &[Message]) -> String {
        let boundaries = self.history_manager.read().find_round_boundaries();
        if boundaries.len() <= self.config.min_retain_rounds {
            return self.generate_simple_summary(history);
        }
        let keep_from = boundaries[boundaries.len() - self.config.min_retain_rounds];
        let to_compress = &history[..keep_from];
        if to_compress.is_empty() {
            return self.generate_simple_summary(history);
        }
        let history_text: String = to_compress
            .iter()
            .map(|m| {
                format!(
                    "[{}]:{}",
                    m.role.as_str(),
                    if m.content.len() > 500 {
                        format!("{}...", &m.content[..500])
                    } else {
                        m.content.clone()
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let msgs = vec![{
            let mut m = HashMap::new();
            m.insert("role".into(), serde_json::json!("user"));
            m.insert(
                "content".into(),
                serde_json::json!(format!("请压缩：\n\n{}", history_text)),
            );
            m
        }];
        match self.llm.invoke(&msgs) {
            Ok(r) => format!(
                "## 历史摘要({}条)\n{}\n---\n(保留最近{}轮)",
                to_compress.len(),
                r.content,
                self.config.min_retain_rounds
            ),
            Err(_) => self.generate_simple_summary(history),
        }
    }

    pub fn build_tool_schemas(&self) -> Vec<HashMap<String, serde_json::Value>> {
        self.tool_registry
            .as_ref()
            .map(|r| {
                r.get_all_tools()
                    .iter()
                    .map(|t| t.to_openai_schema())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn execute_tool_call(
        &self,
        tool_name: &str,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> String {
        match self.tool_registry.as_ref() {
            Some(reg) => {
                let resp = reg.execute_tool(
                    tool_name,
                    &serde_json::to_string(arguments).unwrap_or_default(),
                );
                if resp.is_error() {
                    format!(
                        "❌ [{}]:{}",
                        resp.error_info
                            .as_ref()
                            .map(|e| e.code.as_str())
                            .unwrap_or("UNKNOWN"),
                        resp.text
                    )
                } else if resp.is_partial() {
                    format!("⚠️ {}", resp.text)
                } else {
                    resp.text
                }
            }
            None => "❌ 未配置工具注册表".into(),
        }
    }

    fn register_task_tool(&self) {
        if let Some(ref reg) = self.tool_registry {
            use crate::hello_agent::agents::factory::default_subagent_factory;
            use crate::hello_agent::tools::builtin::task_tool::TaskTool;
            let llm = self.llm.clone();
            let reg_c = reg.clone();
            let cfg = self.config.clone();
            let factory = Arc::new(move |t: &str| {
                default_subagent_factory(t, llm.clone(), Some(reg_c.clone()), Some(cfg.clone()))
                    .unwrap_or_else(|_| {
                        Box::new(crate::hello_agent::agents::simple_agent::SimpleAgent::new(
                            "fallback",
                            llm.clone(),
                            None,
                            cfg.clone(),
                            Some(reg_c.clone()),
                            false,
                            3,
                        ))
                    })
            });
            reg.register_tool(
                Arc::new(TaskTool::new(
                    factory,
                    Some(reg.clone()),
                    Some(self.config.clone()),
                )),
                false,
            );
        }
    }

    fn register_todowrite_tool(&self) {
        if let Some(ref reg) = self.tool_registry {
            reg.register_tool(
                Arc::new(
                    crate::hello_agent::tools::builtin::todowrite_tool::TodoWriteTool::new(
                        ".",
                        &self.config.todowrite_persistence_dir,
                    ),
                ),
                false,
            );
        }
    }

    fn register_devlog_tool(&self) {
        if let Some(ref reg) = self.tool_registry {
            let sid = self
                .trace_logger
                .as_ref()
                .map(|tl| tl.read().get_session_id().to_string())
                .unwrap_or_else(|| {
                    format!(
                        "s-{}-{}",
                        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
                        &uuid::Uuid::new_v4().to_string()[..4]
                    )
                });
            reg.register_tool(
                Arc::new(
                    crate::hello_agent::tools::builtin::devlog_tool::DevLogTool::new(
                        &sid,
                        &self.name,
                        ".",
                        &self.config.devlog_persistence_dir,
                    ),
                ),
                false,
            );
        }
    }
}
