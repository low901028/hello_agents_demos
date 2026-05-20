use chrono::Utc;
use regex::Regex;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use uuid::Uuid;

pub struct TraceLogger {
    output_dir: PathBuf,
    pub session_id: String,
    sanitize: bool,
    jsonl_writer: Option<BufWriter<File>>,
    html_writer: Option<BufWriter<File>>,
    pub events: Vec<serde_json::Value>,
}

impl TraceLogger {
    pub fn new(
        output_dir: &str,
        sanitize: bool,
        _html_include_raw: bool,
    ) -> Result<Self, std::io::Error> {
        let dir = PathBuf::from(output_dir);
        fs::create_dir_all(&dir)?;
        let sid = format!(
            "s-{}-{}",
            Utc::now().format("%Y%m%d-%H%M%S"),
            &Uuid::new_v4().to_string()[..4]
        );
        let jp = dir.join(format!("trace-{}.jsonl", sid));
        let hp = dir.join(format!("trace-{}.html", sid));
        let jw = BufWriter::new(File::create(&jp)?);
        let hw = BufWriter::new(File::create(&hp)?);
        let mut logger = TraceLogger {
            output_dir: dir,
            session_id: sid,
            sanitize,
            jsonl_writer: Some(jw),
            html_writer: Some(hw),
            events: Vec::new(),
        };
        logger.write_html_header()?;
        Ok(logger)
    }

    pub fn get_session_id(&self) -> &str {
        &self.session_id
    }

    pub fn log_event(
        &mut self,
        event: &str,
        payload: &HashMap<String, serde_json::Value>,
        step: Option<usize>,
    ) {
        let mut obj = serde_json::Map::new();
        obj.insert("ts".into(), serde_json::json!(Utc::now().to_rfc3339()));
        obj.insert("session_id".into(), serde_json::json!(&self.session_id));
        obj.insert("event".into(), serde_json::json!(event));
        obj.insert(
            "payload".into(),
            serde_json::to_value(payload).unwrap_or_default(),
        );
        if let Some(s) = step {
            obj.insert("step".into(), serde_json::json!(s));
        }
        let mut val = serde_json::Value::Object(obj);
        if self.sanitize {
            val = Self::sanitize(&val);
        }
        self.events.push(val.clone());
        if let Some(ref mut w) = self.jsonl_writer {
            let _ = writeln!(w, "{}", serde_json::to_string(&val).unwrap_or_default());
            let _ = w.flush();
        }
        self.write_html_event(&val);
    }

    fn sanitize(val: &serde_json::Value) -> serde_json::Value {
        let s = serde_json::to_string(val).unwrap_or_default();
        let s = Regex::new(r"sk-[a-zA-Z0-9]+")
            .unwrap()
            .replace_all(&s, "sk-***")
            .to_string();
        serde_json::from_str(&s).unwrap_or_else(|_| val.clone())
    }

    pub fn finalize(&mut self) {
        let stats = self.compute_stats();
        self.write_html_footer(&stats);
        self.jsonl_writer = None;
        self.html_writer = None;
        println!("✅ Trace已保存: trace-{}", self.session_id);
    }

    fn compute_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut s = HashMap::new();
        s.insert("total_events".into(), serde_json::json!(self.events.len()));
        s
    }

    fn write_html_header(&mut self) -> std::io::Result<()> {
        if let Some(ref mut w) = self.html_writer {
            w.write_all(format!("<!DOCTYPE html><html><head><meta charset=\"UTF-8\"><title>Trace:{}</title></head><body><h1>Trace:{}</h1>", self.session_id, self.session_id).as_bytes())?;
            w.flush()?;
        }
        Ok(())
    }

    fn write_html_event(&mut self, event: &serde_json::Value) {
        if let Some(ref mut w) = self.html_writer {
            let _ = writeln!(
                w,
                "<pre>{}</pre>",
                serde_json::to_string_pretty(event).unwrap_or_default()
            );
            let _ = w.flush();
        }
    }

    fn write_html_footer(&mut self, stats: &HashMap<String, serde_json::Value>) {
        if let Some(ref mut w) = self.html_writer {
            let _ = writeln!(
                w,
                "<hr><h2>统计</h2><pre>{}</pre></body></html>",
                serde_json::to_string_pretty(stats).unwrap_or_default()
            );
            let _ = w.flush();
        }
    }
}
