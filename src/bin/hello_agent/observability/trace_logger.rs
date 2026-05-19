use chrono::Utc;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

/// 双格式 Trace 记录器
pub struct TraceLogger {
    output_dir: PathBuf,
    session_id: String,
    sanitize: bool,
    html_include_raw: bool,
    events: Vec<serde_json::Value>,
    jsonl_file: Option<File>,
    html_file: Option<File>,
}

impl TraceLogger {
    pub fn new(output_dir: impl Into<PathBuf>, sanitize: bool, html_include_raw: bool) -> Self {
        let output_dir = output_dir.into();
        fs::create_dir_all(&output_dir).ok();

        let session_id = Self::generate_session_id();

        let mut logger = Self {
            output_dir,
            session_id,
            sanitize,
            html_include_raw,
            events: Vec::new(),
            jsonl_file: None,
            html_file: None,
        };

        // 打开文件
        logger.jsonl_file = Some(
            File::create(logger.jsonl_path()).unwrap(),
        );
        logger.html_file = Some(
            File::create(logger.html_path()).unwrap(),
        );

        logger.write_html_header();
        logger
    }

    fn generate_session_id() -> String {
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
        let suffix = Uuid::new_v4().to_string()[..4].to_string();
        format!("s-{}-{}", timestamp, suffix)
    }

    fn jsonl_path(&self) -> PathBuf {
        self.output_dir.join(format!("trace-{}.jsonl", self.session_id))
    }

    fn html_path(&self) -> PathBuf {
        self.output_dir.join(format!("trace-{}.html", self.session_id))
    }

    /// 记录事件
    pub fn log_event(&mut self, event: &str, payload: serde_json::Value) {
        let mut event_obj = serde_json::json!({
            "ts": Utc::now().to_rfc3339(),
            "session_id": self.session_id,
            "event": event,
            "payload": payload,
        });

        if self.sanitize {
            event_obj = Self::sanitize_event(event_obj);
        }

        self.events.push(event_obj.clone());

        // 写入 JSONL
        if let Some(ref mut file) = self.jsonl_file {
            writeln!(file, "{}", serde_json::to_string(&event_obj).unwrap_or_default()).ok();
        }

        // 写入 HTML 事件
        self.write_html_event(&event_obj);
    }

    fn sanitize_event(mut event: serde_json::Value) -> serde_json::Value {
        if let Some(payload) = event.get_mut("payload") {
            *payload = Self::sanitize_value(payload.clone());
        }
        event
    }

    fn sanitize_value(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => {
                let s = s.replace(
                    |c: char| c.is_ascii_alphanumeric() || c == '-',
                    "",
                );
                serde_json::Value::String(s)
            }
            serde_json::Value::Object(map) => {
                serde_json::Value::Object(
                    map.into_iter()
                        .map(|(k, v)| (k, Self::sanitize_value(v)))
                        .collect(),
                )
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(Self::sanitize_value).collect())
            }
            other => other,
        }
    }

    fn write_html_header(&mut self) {
        let header = format!(
            r#"<!DOCTYPE html><html><head><meta charset="UTF-8"><title>Trace: {}</title></head><body><h1>Trace Session: {}</h1>"#,
            self.session_id, self.session_id
        );
        if let Some(ref mut file) = self.html_file {
            write!(file, "{}", header).ok();
        }
    }

    fn write_html_event(&mut self, event: &serde_json::Value) {
        let event_type = event["event"].as_str().unwrap_or("unknown");
        let html = format!(
            "<div class='event'><strong>{}</strong>: <pre>{}</pre></div>",
            event_type,
            serde_json::to_string_pretty(&event["payload"]).unwrap_or_default()
        );
        if let Some(ref mut file) = self.html_file {
            write!(file, "{}", html).ok();
        }
    }

    fn write_html_footer(&mut self) {
        if let Some(ref mut file) = self.html_file {
            write!(file, "</body></html>").ok();
        }
    }

    /// 完成记录
    pub fn finalize(&mut self) {
        self.write_html_footer();
        println!("✅ Trace 已保存:");
        println!("   JSONL: {}", self.jsonl_path().display());
        println!("   HTML:  {}", self.html_path().display());
    }
}

impl Drop for TraceLogger {
    fn drop(&mut self) {
        self.write_html_footer();
    }
}