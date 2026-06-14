// src/context/truncator.rs
// 工具输出截断器

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruncateResult {
    pub truncated: bool,
    pub preview: String,
    pub full_output_path: Option<String>,
    pub stats: TruncateStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruncateStats {
    pub direction: Option<String>,
    pub original_lines: usize,
    pub original_bytes: usize,
    pub kept_lines: Option<usize>,
    pub kept_bytes: Option<usize>,
    pub time_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncateDirection {
    Head,
    Tail,
    HeadTail,
}

impl TruncateDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            TruncateDirection::Head => "head",
            TruncateDirection::Tail => "tail",
            TruncateDirection::HeadTail => "head_tail",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "tail" => TruncateDirection::Tail,
            "head_tail" => TruncateDirection::HeadTail,
            _ => TruncateDirection::Head,
        }
    }
}

pub struct ObservationTruncator {
    max_lines: usize,
    max_bytes: usize,
    truncate_direction: TruncateDirection,
    output_dir: PathBuf,
}

impl ObservationTruncator {
    pub fn new(max_lines: usize, max_bytes: usize, direction: TruncateDirection, output_dir: impl Into<PathBuf>) -> Self {
        let output_dir = output_dir.into();
        fs::create_dir_all(&output_dir).ok();
        Self { max_lines, max_bytes, truncate_direction: direction, output_dir }
    }

    pub fn truncate(&self, tool_name: &str, output: &str, metadata: Option<HashMap<String, Value>>) -> TruncateResult {
        let start = Instant::now();
        let lines: Vec<&str> = output.lines().collect();
        let bytes_size = output.as_bytes().len();

        if lines.len() <= self.max_lines && bytes_size <= self.max_bytes {
            return TruncateResult {
                truncated: false,
                preview: output.to_string(),
                full_output_path: None,
                stats: TruncateStats {
                    direction: None,
                    original_lines: lines.len(),
                    original_bytes: bytes_size,
                    kept_lines: None,
                    kept_bytes: None,
                    time_ms: start.elapsed().as_millis() as u64,
                },
            };
        }

        let truncated_lines = self.truncate_lines(&lines);
        let preview = truncated_lines.join("\n");
        let truncated_bytes = preview.as_bytes().len();
        let output_path = self.save_full_output(tool_name, output, metadata);

        TruncateResult {
            truncated: true,
            preview,
            full_output_path: Some(output_path),
            stats: TruncateStats {
                direction: Some(self.truncate_direction.as_str().to_string()),
                original_lines: lines.len(),
                original_bytes: bytes_size,
                kept_lines: Some(truncated_lines.len()),
                kept_bytes: Some(truncated_bytes),
                time_ms: start.elapsed().as_millis() as u64,
            },
        }
    }

    fn truncate_lines<'a>(&self, lines: &[&'a str]) -> Vec<&'a str> {
        match self.truncate_direction {
            TruncateDirection::Head => lines[..self.max_lines.min(lines.len())].to_vec(),
            TruncateDirection::Tail => {
                let start = if self.max_lines >= lines.len() { 0 } else { lines.len() - self.max_lines };
                lines[start..].to_vec()
            }
            TruncateDirection::HeadTail => {
                let half = self.max_lines / 2;
                if lines.len() <= self.max_lines { return lines.to_vec(); }
                let mut res = Vec::new();
                res.extend_from_slice(&lines[..half]);
                res.push("...(中间省略)...");
                res.extend_from_slice(&lines[lines.len() - half..]);
                res
            }
        }
    }

    fn save_full_output(&self, tool_name: &str, output: &str, metadata: Option<HashMap<String, Value>>) -> String {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S_%f");
        let filename = format!("tool_{}_{}.json", timestamp, tool_name);
        let filepath = self.output_dir.join(&filename);

        let data = serde_json::json!({
            "tool": tool_name,
            "output": output,
            "timestamp": Local::now().to_rfc3339(),
            "metadata": metadata.unwrap_or_default()
        });

        if let Ok(mut file) = fs::File::create(&filepath) {
            let _ = file.write_all(serde_json::to_string_pretty(&data).unwrap_or_default().as_bytes());
        }

        filepath.to_string_lossy().to_string()
    }
}