use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum TruncateDirection {
    Head,
    Tail,
    HeadTail,
}

impl TruncateDirection {
    pub fn from_str(s: &str) -> Self {
        match s {
            "tail" => TruncateDirection::Tail,
            "head_tail" => TruncateDirection::HeadTail,
            _ => TruncateDirection::Head,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObservationTruncator {
    max_lines: usize,
    max_bytes: usize,
    direction: TruncateDirection,
    output_dir: PathBuf,
}

impl ObservationTruncator {
    pub fn new(max_lines: usize, max_bytes: usize, direction: &str, output_dir: &str) -> Self {
        let d = PathBuf::from(output_dir);
        fs::create_dir_all(&d).ok();
        ObservationTruncator {
            max_lines,
            max_bytes,
            direction: TruncateDirection::from_str(direction),
            output_dir: d,
        }
    }
    pub fn truncate(
        &self,
        tool_name: &str,
        output: &str,
        _metadata: Option<&HashMap<String, serde_json::Value>>,
    ) -> HashMap<String, serde_json::Value> {
        let start = Instant::now();
        let lines: Vec<&str> = output.lines().collect();
        let bytes = output.len();
        if lines.len() <= self.max_lines && bytes <= self.max_bytes {
            let mut r = HashMap::new();
            r.insert("truncated".into(), serde_json::json!(false));
            r.insert("preview".into(), serde_json::json!(output));
            r.insert("stats".into(), serde_json::json!({"original_lines":lines.len(),"original_bytes":bytes,"time_ms":start.elapsed().as_millis() as i64}));
            return r;
        }
        let truncated = match self.direction {
            TruncateDirection::Head => lines[..self.max_lines.min(lines.len())].join("\n"),
            TruncateDirection::Tail => {
                lines[lines.len().saturating_sub(self.max_lines)..].join("\n")
            }
            TruncateDirection::HeadTail => {
                let half = self.max_lines / 2;
                if lines.len() <= self.max_lines {
                    lines.join("\n")
                } else {
                    format!(
                        "{}\n...(省略)...\n{}",
                        lines[..half].join("\n"),
                        lines[lines.len() - half..].join("\n")
                    )
                }
            }
        };
        let ts = Utc::now().format("%Y%m%d_%H%M%S_%f");
        let fp = self
            .output_dir
            .join(format!("tool_{}_{}.json", ts, tool_name));
        if let Ok(mut f) = fs::File::create(&fp) {
            let _ = f.write_all(
                serde_json::to_string(&serde_json::json!({"tool":tool_name,"output":output}))
                    .unwrap()
                    .as_bytes(),
            );
        }
        let mut r = HashMap::new();
        r.insert("truncated".into(), serde_json::json!(true));
        r.insert("preview".into(), serde_json::json!(truncated));
        r.insert(
            "full_output_path".into(),
            serde_json::json!(fp.to_string_lossy()),
        );
        r
    }
}
