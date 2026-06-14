// trace_logger.rs
//!双格式 Trace Logger
//!
//!     特性：
//!     - JSONL 流式写入（实时追加）
//!     - HTML 增量渲染（实时可查看）
//!     - 自动脱敏（API Key、路径）
//!     - 内置统计面板（Token、工具调用、错误）
//!
//!     使用示例：
//! ```rust
//!     let mut trace_logger = TraceLogger::new("memory/traces", false, Some(true));
//!     // let trace_logger = TraceLogger::new("memory/traces", false, None);
//!     // let trace_logger = TraceLogger::new("memory/traces", true, Some(true));
//!     trace_logger.log_event("session_start", serde_json::json!({"agent_name": "MyAgent"}), None);
//!     trace_logger.log_event("tool_call", serde_json::json!({"tool_name": "Calculator"}), Some(1));
//!     trace_logger.finalize();  // 生成最终 HTML
//! ```
//!
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::Local;
use regex::Regex;
use serde_json::{json, Value};
use uuid::Uuid;

/// 双格式 Trace 记录器
///
/// 输出格式：
/// - JSONL: 机器可读，流式追加
/// - HTML: 人类可读，可视化界面，内置统计面板
pub struct TraceLogger {
    output_dir: PathBuf,
    sanitize: bool,
    html_include_raw: bool,
    session_id: String,
    events: Vec<Value>,
    jsonl_writer: BufWriter<File>,
    html_writer: BufWriter<File>,
    jsonl_path: PathBuf,
    html_path: PathBuf,
    finalized: bool,
}

impl TraceLogger {
    /// 初始化 TraceLogger
    ///
    /// # Arguments
    /// * `output_dir` - 输出目录
    /// * `sanitize` - 是否脱敏敏感信息
    /// * `html_include_raw_response` - HTML 是否包含原始响应
    pub fn new(
        output_dir: impl AsRef<Path>,
        sanitize: bool,
        html_include_raw: Option<bool>,
    ) -> Self {
        let output_dir = output_dir.as_ref().to_path_buf();
        fs::create_dir_all(&output_dir).expect("创建输出目录失败");

        let session_id = Self::generate_session_id();
        let jsonl_path = output_dir.join(format!("trace-{}.jsonl", session_id));
        let html_path = output_dir.join(format!("trace-{}.html", session_id));

        let jsonl_file = File::create(&jsonl_path).expect("创建JSONL文件失败");
        let html_file = File::create(&html_path).expect("创建HTML文件失败");

        let jsonl_writer = BufWriter::new(jsonl_file);
        let mut html_writer = BufWriter::new(html_file);

        // 写入 HTML 头部
        Self::write_html_header(&mut html_writer, &session_id);

        Self {
            output_dir,
            sanitize,
            html_include_raw: html_include_raw.unwrap_or(false),
            session_id,
            events: Vec::new(),
            jsonl_writer,
            html_writer,
            jsonl_path,
            html_path,
            finalized: false,
        }
    }

    fn generate_session_id() -> String {
        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        let suffix = &Uuid::new_v4().to_string()[..4];
        format!("s-{}-{}", timestamp, suffix)
    }

    /// 记录事件
    ///
    /// # Arguments
    /// * `event` - 事件类型
    /// * `payload` - 事件数据
    /// * `step` - 步骤序号（可选）
    pub fn log_event(&mut self, event: &str, payload: Value, step: Option<usize>) {
        let mut event_obj = json!({
            "ts": Local::now().to_rfc3339(),
            "session_id": self.session_id,
            "event": event,
            "payload": payload,
        });
        if let Some(step) = step {
            event_obj["step"] = json!(step);
        }

        // 脱敏
        if self.sanitize {
            event_obj = Self::sanitize_event(event_obj);
        }

        // 缓存
        self.events.push(event_obj.clone());

        // 写入 JSONL
        writeln!(
            self.jsonl_writer,
            "{}",
            serde_json::to_string(&event_obj).expect("序列化事件失败")
        )
            .expect("写入JSONL失败");
        self.jsonl_writer.flush().expect("刷新JSONL失败");

        // 写入 HTML 事件片段
        Self::write_html_event(&mut self.html_writer, &event_obj, self.events.len());
    }

    /// 递归脱敏事件对象
    fn sanitize_event(mut event: Value) -> Value {
        if let Some(payload) = event.get_mut("payload") {
            *payload = Self::sanitize_value(payload.clone());
        }
        event
    }

    fn sanitize_value(value: Value) -> Value {
        match value {
            Value::String(s) => {
                let re_sk = Regex::new(r"sk-[a-zA-Z0-9]+").unwrap();
                let re_bearer = Regex::new(r"Bearer\s+[a-zA-Z0-9_\-]+").unwrap();
                let re_path = Regex::new(r"(/Users/|/home/|C:\\Users\\)[^/\\]+").unwrap();

                let s = re_sk.replace_all(&s, "sk-***").to_string();
                let s = re_bearer.replace_all(&s, "Bearer ***").to_string();
                let s = re_path.replace_all(&s, "${1}***").to_string();
                Value::String(s)
            }
            Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    new_map.insert(k, Self::sanitize_value(v));
                }
                Value::Object(new_map)
            }
            Value::Array(arr) => {
                Value::Array(arr.into_iter().map(Self::sanitize_value).collect())
            }
            other => other,
        }
    }

    /// 完成并关闭记录器
    pub fn finalize(&mut self) {
        if self.finalized {
            return;
        }
        self.finalized = true;

        let stats = self.compute_stats();
        Self::write_html_footer(&mut self.html_writer, &stats);

        // 刷新并关闭文件（BufWriter 在 drop 时会自动关闭）
        self.jsonl_writer.flush().expect("刷新JSONL失败");
        self.html_writer.flush().expect("刷新HTML失败");

        println!("✅ Trace 已保存:");
        println!("   JSONL: {}", self.jsonl_path.display());
        println!("   HTML:  {}", self.html_path.display());
    }

    fn compute_stats(&self) -> Value {
        let mut total_steps = 0;
        let mut total_tokens = 0;
        let mut total_cost = 0.0;
        let mut tool_calls: HashMap<String, usize> = HashMap::new();
        let mut errors: Vec<Value> = Vec::new();
        let mut model_calls = 0;
        let mut session_start: Option<chrono::DateTime<chrono::FixedOffset>> = None;
        let mut session_end: Option<chrono::DateTime<chrono::FixedOffset>> = None;

        for event in &self.events {
            let event_type = event["event"].as_str().unwrap_or("");
            let step = event.get("step").and_then(|s| s.as_u64()).unwrap_or(0) as usize;
            if step > total_steps {
                total_steps = step;
            }

            match event_type {
                "session_start" => {
                    if let Some(ts) = event["ts"].as_str() {
                        session_start = chrono::DateTime::parse_from_rfc3339(ts).ok();
                    }
                }
                "session_end" => {
                    if let Some(ts) = event["ts"].as_str() {
                        session_end = chrono::DateTime::parse_from_rfc3339(ts).ok();
                    }
                }
                "model_output" => {
                    let payload = &event["payload"];
                    if let Some(usage) = payload.get("usage") {
                        total_tokens += usage.get("total_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as usize;
                        total_cost += usage.get("cost").and_then(|c| c.as_f64()).unwrap_or(0.0);
                    }
                    model_calls += 1;
                }
                "tool_call" => {
                    let tool_name = event["payload"]["tool_name"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_string();
                    *tool_calls.entry(tool_name).or_insert(0) += 1;
                }
                "error" => {
                    errors.push(json!({
                        "step": event.get("step"),
                        "type": event["payload"].get("error_type"),
                        "message": event["payload"].get("message"),
                    }));
                }
                _ => {}
            }
        }

        let duration = match (session_start, session_end) {
            (Some(start), Some(end)) => (end - start).num_seconds() as f64,
            _ => 0.0,
        };

        json!({
            "total_steps": total_steps,
            "total_tokens": total_tokens,
            "total_cost": total_cost,
            "tool_calls": tool_calls,
            "errors": errors,
            "duration_seconds": duration,
            "model_calls": model_calls,
        })
    }

    // ---------- HTML 生成辅助方法 ----------
    fn write_html_header(writer: &mut impl Write, session_id: &str) {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S");
        let header = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Trace: {session_id}</title>
    <style>
        body {{
            font-family: 'Consolas', 'Monaco', monospace;
            padding: 20px;
            background: #1a1a1a;
            color: #e0e0e0;
            margin: 0;
        }}
        .header {{
            background: #2a2a2a;
            padding: 20px;
            border-radius: 8px;
            margin-bottom: 20px;
        }}
        .header h1 {{
            margin: 0 0 10px 0;
            color: #4af626;
        }}
        .stats-panel {{
            background: #2a2a2a;
            padding: 20px;
            border-radius: 8px;
            margin-bottom: 20px;
        }}
        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 15px;
            margin-bottom: 20px;
        }}
        .stat-item {{
            background: #1a1a1a;
            padding: 15px;
            border-radius: 5px;
            border-left: 3px solid #4af626;
        }}
        .stat-label {{
            display: block;
            color: #888;
            font-size: 12px;
            margin-bottom: 5px;
        }}
        .stat-value {{
            display: block;
            color: #e0e0e0;
            font-size: 24px;
            font-weight: bold;
        }}
        .tool-stats {{
            width: 100%;
            border-collapse: collapse;
            margin-top: 10px;
        }}
        .tool-stats th, .tool-stats td {{
            padding: 8px;
            text-align: left;
            border-bottom: 1px solid #333;
        }}
        .tool-stats th {{
            color: #4af626;
        }}
        .error-list {{
            list-style: none;
            padding: 0;
        }}
        .error-list li {{
            background: #331111;
            padding: 10px;
            margin: 5px 0;
            border-radius: 5px;
            border-left: 3px solid #ff4444;
        }}
        .events-container {{
            background: #2a2a2a;
            padding: 20px;
            border-radius: 8px;
        }}
        .event {{
            border: 1px solid #333;
            margin: 10px 0;
            padding: 15px;
            border-radius: 5px;
            background: #1a1a1a;
        }}
        .event-header {{
            display: flex;
            align-items: center;
            gap: 10px;
            margin-bottom: 10px;
        }}
        .step {{
            color: #888;
            font-size: 12px;
        }}
        .timestamp {{
            color: #666;
            font-size: 11px;
        }}
        .event-type {{
            color: #4af626;
            font-weight: bold;
        }}
        .expandable {{
            cursor: pointer;
            color: #4af626;
            user-select: none;
        }}
        .expandable:hover {{
            color: #6fff48;
        }}
        .details {{
            display: none;
            margin-top: 10px;
            padding: 10px;
            background: #0d0d0d;
            border-radius: 5px;
            overflow-x: auto;
        }}
        .details pre {{
            margin: 0;
            color: #e0e0e0;
        }}
        .tool-call {{
            border-left: 3px solid #4af626;
        }}
        .tool-result {{
            border-left: 3px solid #ffd700;
        }}
        .error {{
            border-left: 3px solid #ff4444;
            background: #2a1a1a;
        }}
        .model-output {{
            border-left: 3px solid #00bfff;
        }}
    </style>
</head>
<body>
    <div class="header">
        <h1>🔍 Trace Session: {session_id}</h1>
        <p>生成时间: {now}</p>
    </div>

    <div class="events-container">
        <h2>📋 事件列表</h2>
"#,
            session_id = session_id,
            now = now,
        );
        write!(writer, "{}", header).expect("写入HTML头部失败");
    }

    fn write_html_event(writer: &mut impl Write, event: &Value, event_index: usize) {
        let event_type = event["event"].as_str().unwrap_or("");
        let step = event
            .get("step")
            .and_then(|s| s.as_u64())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "-".to_string());
        let timestamp = event["ts"].as_str().unwrap_or("");
        let payload = &event["payload"];
        let payload_json =
            serde_json::to_string_pretty(payload).unwrap_or_else(|_| "{}".to_string());

        let css_class = match event_type {
            "tool_call" => "event tool-call",
            "tool_result" => "event tool-result",
            "error" => "event error",
            "model_output" => "event model-output",
            _ => "event",
        };

        let details_id = format!("details-{}", event_index);

        let event_html = format!(
            r#"
        <div class="{css_class}">
            <div class="event-header">
                <span class="step">Step {step}</span>
                <span class="timestamp">{timestamp}</span>
                <span class="event-type">{event_type}</span>
                <span class="expandable" onclick="toggleDetails('{details_id}')">[▼ 详情]</span>
            </div>
            <div id="{details_id}" class="details">
                <pre>{payload_json}</pre>
            </div>
        </div>
"#,
            css_class = css_class,
            step = step,
            timestamp = timestamp,
            event_type = event_type,
            details_id = details_id,
            payload_json = payload_json,
        );
        write!(writer, "{}", event_html).expect("写入HTML事件失败");
    }

    fn write_html_footer(writer: &mut impl Write, stats: &Value) {
        // 工具调用统计表格
        let tool_calls = stats["tool_calls"].as_object();
        let mut tool_rows = String::new();
        if let Some(tools) = tool_calls {
            let mut tools_sorted: Vec<_> = tools.iter().collect();
            tools_sorted.sort_by(|a, b| b.1.as_u64().cmp(&a.1.as_u64()));
            for (name, count) in tools_sorted {
                tool_rows.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td></tr>\n",
                    name, count
                ));
            }
        }
        if tool_rows.is_empty() {
            tool_rows = "<tr><td colspan=\"2\">无工具调用</td></tr>".to_string();
        }

        // 错误列表
        let errors = stats["errors"].as_array();
        let mut error_html = String::new();
        if let Some(errs) = errors {
            if !errs.is_empty() {
                let mut error_items = String::new();
                for err in errs {
                    let step = err["step"].as_u64().map(|s| s.to_string()).unwrap_or_else(|| "?".to_string());
                    let error_type = err["type"].as_str().unwrap_or("UNKNOWN");
                    let message = err["message"].as_str().unwrap_or("");
                    error_items.push_str(&format!(
                        "<li>Step {}: <strong>{}</strong> - {}</li>\n",
                        step, error_type, message
                    ));
                }
                error_html = format!(
                    r#"
        <h3>❌ 错误列表 ({})</h3>
        <ul class="error-list">
            {}
        </ul>
"#,
                    errs.len(),
                    error_items
                );
            }
        }

        let total_steps = stats["total_steps"].as_u64().unwrap_or(0);
        let total_tokens = stats["total_tokens"].as_u64().unwrap_or(0);
        let total_cost = stats["total_cost"].as_f64().unwrap_or(0.0);
        let duration = stats["duration_seconds"].as_f64().unwrap_or(0.0);
        let model_calls = stats["model_calls"].as_u64().unwrap_or(0);

        let footer = format!(
            r#"
    </div>

    <div class="stats-panel">
        <h2>📊 会话统计</h2>
        <div class="stats-grid">
            <div class="stat-item">
                <span class="stat-label">总步骤数</span>
                <span class="stat-value">{total_steps}</span>
            </div>
            <div class="stat-item">
                <span class="stat-label">总 Token</span>
                <span class="stat-value">{total_tokens}</span>
            </div>
            <div class="stat-item">
                <span class="stat-label">总成本</span>
                <span class="stat-value">${total_cost:.4}</span>
            </div>
            <div class="stat-item">
                <span class="stat-label">会话时长</span>
                <span class="stat-value">{duration:.1}s</span>
            </div>
            <div class="stat-item">
                <span class="stat-label">模型调用次数</span>
                <span class="stat-value">{model_calls}</span>
            </div>
        </div>

        <h3>🔧 工具调用统计</h3>
        <table class="tool-stats">
            <tr><th>工具名称</th><th>调用次数</th></tr>
            {tool_rows}
        </table>

        {error_html}
    </div>

    <script>
        function toggleDetails(id) {{
            const el = document.getElementById(id);
            if (el.style.display === 'none' || el.style.display === '') {{
                el.style.display = 'block';
            }} else {{
                el.style.display = 'none';
            }}
        }}
    </script>
</body>
</html>
"#,
            total_steps = total_steps,
            total_tokens = total_tokens,
            total_cost = total_cost,
            duration = duration,
            model_calls = model_calls,
            tool_rows = tool_rows,
            error_html = error_html,
        );
        write!(writer, "{}", footer).expect("写入HTML尾部失败");
    }
}

impl Drop for TraceLogger {
    fn drop(&mut self) {
        if !self.finalized {
            // 尝试 finalize，但忽略错误（避免在 panic 期间再次 panic）
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.finalize();
            }));
        }
    }
}