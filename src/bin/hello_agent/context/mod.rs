//! 上下文工程模块
//!
//! 为HelloAgents框架提供上下文工程能力：
//! - ContextBuilder: GSSC流水线
//! - HistoryManager: 历史管理与压缩
//! - ObservationTruncator: 工具输出截断
//! - TokenCounter: Token 计数器
//!
pub mod builder;
pub mod history;
pub mod token_counter;
pub mod truncator;
