// examples/session_demo.rs
// 会话持久化使用示例 (异步版)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tempfile::TempDir;
use uuid::Uuid;

use hello_agents::context::history_manager_impl::HistoryManagerImpl;
use hello_agents::core::traits::history::HistoryManager;
use hello_agents::core::traits::session_store::SessionStore;
use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::core::types::message::Message;
use hello_agents::core::types::session::{SessionData, SessionInfo};
// ==================== 简单的文件会话存储实现 ====================

pub struct FileSessionStore {
    session_dir: PathBuf,
}

impl FileSessionStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { session_dir: dir.into() }
    }
}

#[async_trait]
impl SessionStore for FileSessionStore {
    async fn save(&self, session: &SessionData) -> Result<String, HelloAgentError> {
        let filename = session.session_id.clone() + ".json";
        let filepath = self.session_dir.join(&filename);
        let tmp = filepath.with_extension("tmp");
        let json = serde_json::to_vec_pretty(session)?;
        tokio::fs::write(&tmp, json).await?;
        tokio::fs::rename(&tmp, &filepath).await?;
        Ok(filepath.to_string_lossy().into_owned())
    }

    async fn load(&self, path: &str) -> Result<SessionData, HelloAgentError> {
        let data = tokio::fs::read_to_string(path).await?;
        let session: SessionData = serde_json::from_str(&data)?;
        Ok(session)
    }

    async fn list_sessions(&self) -> Result<Vec<SessionInfo>, HelloAgentError> {
        let mut sessions = Vec::new();
        let mut dir = tokio::fs::read_dir(&self.session_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(data) = tokio::fs::read_to_string(&path).await {
                    if let Ok(session) = serde_json::from_str::<SessionData>(&data) {
                        sessions.push(SessionInfo {
                            filename: entry.file_name().to_string_lossy().into_owned(),
                            session_id: session.session_id,
                            created_at: session.metadata.get("created_at")
                                .and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        });
                    }
                }
            }
        }
        Ok(sessions)
    }
}

// ==================== 演示函数 ====================

async fn demo_basic_save_load() -> Result<(), HelloAgentError> {
    println!("{}", "=".repeat(60));
    println!("示例 1: 基本保存和加载");
    println!("{}", "=".repeat(60));

    let tmp_dir = TempDir::new().unwrap();
    let session_store = Arc::new(FileSessionStore::new(tmp_dir.path()));
    let mut history = HistoryManagerImpl::new(10, 0.8);

    // 添加对话历史
    println!("\n添加对话历史...");
    history.add_message(Message::user("你好"));
    history.add_message(Message::assistant("你好！有什么可以帮助你的？"));
    history.add_message(Message::user("介绍一下你自己"));
    history.add_message(Message::assistant("我是 AI 助手"));

    println!("当前历史长度: {}", history.messages().len());

    // 构造会话数据
    let session_id = Uuid::new_v4().to_string();
    let session = SessionData {
        session_id: session_id.clone(),
        history: history.messages().to_vec(),
        metadata: {
            let mut m = HashMap::new();
            m.insert("created_at".to_string(), json!(Utc::now().to_rfc3339()));
            m
        },
    };

    // 保存会话
    println!("\n保存会话...");
    let filepath = session_store.save(&session).await?;
    println!("会话已保存: {}", filepath);

    // 清空历史
    history.clear();
    println!("清空后历史长度: {}", history.messages().len());

    // 加载会话
    println!("\n加载会话...");
    let loaded = session_store.load(&filepath).await?;
    // 恢复历史
    history.clear();
    for msg in loaded.history {
        history.add_message(msg);
    }
    println!("恢复后历史长度: {}", history.messages().len());

    // 验证内容
    let msgs = history.messages();
    assert_eq!(msgs.len(), 4);
    assert_eq!(msgs[0].content.as_ref().unwrap(), "你好");

    println!("\n✅ 基本保存和加载测试完成");
    Ok(())
}

async fn demo_list_sessions() -> Result<(), HelloAgentError> {
    println!("\n{}", "=".repeat(60));
    println!("示例 2: 列出所有会话");
    println!("{}", "=".repeat(60));

    let tmp_dir = TempDir::new().unwrap();
    let session_store = Arc::new(FileSessionStore::new(tmp_dir.path()));

    // 创建多个会话
    println!("\n创建多个会话...");
    for i in 1..=3 {
        let session_id = format!("session-{}", i);
        let session = SessionData {
            session_id,
            history: vec![Message::user(&format!("消息 {}", i))],
            metadata: {
                let mut m = HashMap::new();
                m.insert("created_at".to_string(), json!(Utc::now().to_rfc3339()));
                m
            },
        };
        session_store.save(&session).await?;
    }

    // 列出所有会话
    println!("\n列出所有会话:");
    let sessions = session_store.list_sessions().await?;
    for s in &sessions {
        println!("\n  会话 ID: {}", s.session_id);
        println!("  创建时间: {}", s.created_at);
        println!("  文件: {}", s.filename);
    }
    assert_eq!(sessions.len(), 3);
    println!("\n✅ 列出会话测试完成");
    Ok(())
}

async fn demo_consistency_check() -> Result<(), HelloAgentError> {
    println!("\n{}", "=".repeat(60));
    println!("示例 3: 环境一致性检查");
    println!("{}", "=".repeat(60));

    let tmp_dir = TempDir::new().unwrap();
    let session_store = Arc::new(FileSessionStore::new(tmp_dir.path()));

    // 模拟两个不同的工具配置（这里只是演示概念）
    let session = SessionData {
        session_id: "consistency-test".into(),
        history: vec![Message::user("测试消息")],
        metadata: {
            let mut m = HashMap::new();
            m.insert("created_at".to_string(), json!(Utc::now().to_rfc3339()));
            m.insert("tool_schema_hash".to_string(), json!("hash-v1"));
            m.insert("agent_config".to_string(), json!({"llm_model": "gpt-4"}));
            m
        },
    };

    let filepath = session_store.save(&session).await?;
    println!("\n会话已保存: {}", Path::new(&filepath).file_name().unwrap().to_string_lossy());

    // 加载会话，检查一致性
    let loaded = session_store.load(&filepath).await?;
    println!("\n加载会话...");
    // 在实际应用中，我们会比较当前的工具 schema 哈希和 agent config
    if let Some(saved_config) = loaded.metadata.get("agent_config") {
        let current_model = "gpt-4"; // 模拟当前配置
        if saved_config["llm_model"] != json!(current_model) {
            println!("⚠️ 模型配置不一致！");
        }
    }
    if let Some(saved_hash) = loaded.metadata.get("tool_schema_hash") {
        let current_hash = "hash-v2"; // 模拟当前哈希
        if *saved_hash != json!(current_hash) {
            println!("⚠️ 工具 schema 已变更！");
        }
    }
    println!("\n✅ 环境一致性检查完成");
    Ok(())
}

async fn demo_auto_save() -> Result<(), HelloAgentError> {
    println!("\n{}", "=".repeat(60));
    println!("示例 4: 自动保存机制");
    println!("{}", "=".repeat(60));

    let tmp_dir = TempDir::new().unwrap();
    let session_store = Arc::new(FileSessionStore::new(tmp_dir.path()));
    let mut history = HistoryManagerImpl::new(10, 0.8);
    let auto_save_interval = 3;

    println!("\n添加消息（每 {} 条自动保存）...", auto_save_interval);
    let mut msg_count = 0;
    for i in 0..7 {
        history.add_message(Message::user(&format!("消息 {}", i + 1)));
        msg_count += 1;
        println!("  添加消息 {}", i + 1);

        if msg_count % auto_save_interval == 0 {
            let session = SessionData {
                session_id: "auto-save".into(),
                history: history.messages().to_vec(),
                metadata: HashMap::new(),
            };
            session_store.save(&session).await?;
            println!("    -> 自动保存");
        }
    }

    // 检查自动保存的文件
    let mut count = 0;
    let mut dir = tokio::fs::read_dir(tmp_dir.path()).await?;
    while let Some(_) = dir.next_entry().await? {
        count += 1;
    }
    println!("\n自动保存文件数: {}", count);
    assert!(count >= 1);
    println!("\n✅ 自动保存测试完成");
    Ok(())
}

async fn demo_metadata_tracking() -> Result<(), HelloAgentError> {
    println!("\n{}", "=".repeat(60));
    println!("示例 5: 会话元数据追踪");
    println!("{}", "=".repeat(60));

    let tmp_dir = TempDir::new().unwrap();
    let session_store = Arc::new(FileSessionStore::new(tmp_dir.path()));

    let mut history = HistoryManagerImpl::new(10, 0.8);
    for i in 0..5 {
        history.add_message(Message::user(&format!("消息 {}", i + 1)));
        history.add_message(Message::assistant(&format!("回复 {}", i + 1)));
    }

    let start = Utc::now();
    // 模拟一些处理时间
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    let end = Utc::now();
    let duration = (end - start).num_seconds() as f64;

    let mut metadata = HashMap::new();
    metadata.insert("total_tokens".to_string(), json!(history.estimate_tokens()));
    metadata.insert("total_steps".to_string(), json!(5));
    metadata.insert("duration_seconds".to_string(), json!(duration));

    let session = SessionData {
        session_id: "metadata-test".into(),
        history: history.messages().to_vec(),
        metadata: metadata.clone(),
    };

    let filepath = session_store.save(&session).await?;
    let loaded = session_store.load(&filepath).await?;

    println!("\n会话元数据:");
    println!("  会话 ID: {}", loaded.session_id);
    if let Some(created) = loaded.metadata.get("created_at") {
        println!("  创建时间: {}", created);
    }
    println!("  历史消息数: {}", loaded.history.len());

    println!("\n统计信息:");
    println!("  总 Tokens: {}", loaded.metadata.get("total_tokens").map_or(0, |v| v.as_u64().unwrap_or(0)));
    println!("  总步数: {}", loaded.metadata.get("total_steps").map_or(0, |v| v.as_u64().unwrap_or(0)));
    println!("  持续时间: {} 秒", loaded.metadata.get("duration_seconds").map_or(0.0, |v| v.as_f64().unwrap_or(0.0)));

    println!("\n✅ 元数据追踪测试完成");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    demo_basic_save_load().await?;
    demo_list_sessions().await?;
    demo_consistency_check().await?;
    demo_auto_save().await?;
    demo_metadata_tracking().await?;

    println!("\n{}", "=".repeat(60));
    println!("✅ 所有示例运行完成！");
    println!("{}", "=".repeat(60));
    Ok(())
}