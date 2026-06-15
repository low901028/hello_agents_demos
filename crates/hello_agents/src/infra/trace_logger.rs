//! ============================================================
//! src/infra/trace_logger.rs   (实现 TraceLogger trait)
//! ============================================================
use crate::core::observability::TraceLogger as TraceLoggerTrait;
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub struct TraceLogger {
    session_id: String,
    output_dir: PathBuf,
    jsonl_file: std::fs::File,
    html_file: std::fs::File,
}

impl TraceLogger {
    pub fn new(session_id: &str, output_dir: &PathBuf) -> Self {
        fs::create_dir_all(output_dir).ok();
        let jsonl_path = output_dir.join(format!("{}.jsonl", session_id));
        let html_path = output_dir.join(format!("{}.html", session_id));
        let jsonl_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&jsonl_path)
            .unwrap();
        let html_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&html_path)
            .unwrap();
        Self {
            session_id: session_id.into(),
            output_dir: output_dir.clone(),
            jsonl_file,
            html_file,
        }
    }
}

impl TraceLoggerTrait for TraceLogger {
    fn log_event(&self, event: &str, payload: Value, step: Option<usize>) {
        let mut record = serde_json::json!({
            "ts": chrono::Utc::now().to_rfc3339(),
            "session_id": self.session_id,
            "event": event,
            "payload": payload,
        });
        if let Some(s) = step {
            record["step"] = serde_json::json!(s);
        }
        let mut file = &self.jsonl_file;
        writeln!(file, "{}", serde_json::to_string(&record).unwrap()).ok();
    }

    fn finalize(&self) {
        // 生成 HTML 统计面板等，此处省略
    }

    fn session_id(&self) -> &str {
        &self.session_id
    }
}
