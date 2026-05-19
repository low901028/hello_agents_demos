use chrono::Utc;
use std::fs;
use std::path::PathBuf;

/// 工具输出截断器
pub struct ObservationTruncator {
    max_lines: usize,
    max_bytes: usize,
    truncate_direction: String,
    output_dir: PathBuf,
}

impl ObservationTruncator {
    pub fn new(
        max_lines: usize,
        max_bytes: usize,
        truncate_direction: impl Into<String>,
        output_dir: impl Into<PathBuf>,
    ) -> Self {
        let dir = output_dir.into();
        fs::create_dir_all(&dir).ok();
        Self {
            max_lines,
            max_bytes,
            truncate_direction: truncate_direction.into(),
            output_dir: dir,
        }
    }

    /// 截断工具输出
    pub fn truncate(
        &self,
        tool_name: &str,
        output: &str,
        metadata: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let lines: Vec<&str> = output.lines().collect();
        let bytes_size = output.len();

        // 无需截断
        if lines.len() <= self.max_lines && bytes_size <= self.max_bytes {
            return serde_json::json!({
                "truncated": false,
                "preview": output,
                "full_output_path": null,
                "stats": {
                    "original_lines": lines.len(),
                    "original_bytes": bytes_size,
                }
            });
        }

        // 需要截断
        let truncated_lines = self.truncate_lines(&lines);
        let preview = truncated_lines.join("\n");

        // 保存完整输出
        let output_path = self.save_full_output(tool_name, output, metadata);

        serde_json::json!({
            "truncated": true,
            "preview": preview,
            "full_output_path": output_path,
            "stats": {
                "direction": self.truncate_direction,
                "original_lines": lines.len(),
                "original_bytes": bytes_size,
                "kept_lines": truncated_lines.len(),
                "kept_bytes": preview.len(),
            }
        })
    }

    fn truncate_lines<'a>(&self, lines: &[&'a str]) -> Vec<&'a str> {
        match self.truncate_direction.as_str() {
            "head" => lines[..self.max_lines.min(lines.len())].to_vec(),
            "tail" => {
                let start = if lines.len() > self.max_lines {
                    lines.len() - self.max_lines
                } else {
                    0
                };
                lines[start..].to_vec()
            }
            "head_tail" => {
                let half = self.max_lines / 2;
                if lines.len() <= half * 2 {
                    lines.to_vec()
                } else {
                    let mut result = lines[..half].to_vec();
                    result.push("...(中间省略)...");
                    result.extend_from_slice(&lines[lines.len() - half..]);
                    result
                }
            }
            _ => lines[..self.max_lines.min(lines.len())].to_vec(),
        }
    }

    fn save_full_output(
        &self,
        tool_name: &str,
        output: &str,
        metadata: Option<serde_json::Value>,
    ) -> String {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S_%f");
        let filename = format!("tool_{}_{}.json", timestamp, tool_name);
        let filepath = self.output_dir.join(&filename);

        let data = serde_json::json!({
            "tool": tool_name,
            "output": output,
            "timestamp": Utc::now().to_rfc3339(),
            "metadata": metadata.unwrap_or_default(),
        });

        if let Ok(json) = serde_json::to_string_pretty(&data) {
            fs::write(&filepath, json).ok();
        }

        filepath.to_string_lossy().to_string()
    }
}