//! truncator.rs
//! 工具输出截断器 - ObservationTruncator - 工具输出截断器
//!
//! 职责：
//! - 统一截断工具输出（避免每个工具自己实现）
//! - 支持多种截断方向（head/tail/head_tail）
//! - 返回 ToolResponse.partial() 状态
//! - 保存完整输出到文件

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 截断结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruncateResult {
    pub truncated: bool,
    pub preview: String,
    pub full_output_path: Option<String>,
    pub stats: TruncateStats,
}

/// 截断统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruncateStats {
    /// 截断方向（仅截断时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    pub original_lines: usize,
    pub original_bytes: usize,
    /// 保留行数（仅截断时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept_lines: Option<usize>,
    /// 保留字节数（仅截断时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept_bytes: Option<usize>,
    /// 处理耗时（毫秒）
    pub time_ms: u64,
}

/// 截断方向
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

/// 工具输出截断器
pub struct ObservationTruncator {
    max_lines: usize,
    max_bytes: usize,
    truncate_direction: TruncateDirection,
    output_dir: PathBuf,
}

impl ObservationTruncator {
    /// 创建新的截断器
    pub fn new(
        max_lines: usize,
        max_bytes: usize,
        truncate_direction: TruncateDirection,
        output_dir: impl Into<PathBuf>,
    ) -> Self {
        let output_dir = output_dir.into();
        // 确保输出目录存在
        fs::create_dir_all(&output_dir).ok();

        Self {
            max_lines,
            max_bytes,
            truncate_direction,
            output_dir,
        }
    }

    /// 截断工具输出
    ///
    /// # Arguments
    /// * `tool_name` - 工具名称
    /// * `output` - 原始输出
    /// * `metadata` - 元数据（可选）
    ///
    /// # Returns
    /// TruncateResult 结构体
    pub fn truncate(
        &self,
        tool_name: &str,
        output: &str,
        metadata: Option<HashMap<String, Value>>,
    ) -> TruncateResult {
        let start = Instant::now();
        let lines: Vec<&str> = output.lines().collect();
        let bytes_size = output.as_bytes().len();

        // 检查是否需要截断
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

        // 需要截断
        let truncated_lines = self._truncate_lines(&lines);
        let preview = truncated_lines.join("\n");
        let truncated_bytes = preview.as_bytes().len();

        // 保存完整输出
        let output_path = self._save_full_output(tool_name, output, metadata);

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

    fn _truncate_lines<'a>(&self, lines: &[&'a str]) -> Vec<&'a str> {
        match self.truncate_direction {
            TruncateDirection::Head => lines[..self.max_lines.min(lines.len())].to_vec(),
            TruncateDirection::Tail => {
                let start = if self.max_lines >= lines.len() { 0 } else { lines.len() - self.max_lines };
                lines[start..].to_vec()
            }
            TruncateDirection::HeadTail => {
                let half = self.max_lines / 2;
                if lines.len() <= self.max_lines {
                    return lines.to_vec();
                }
                let mut res = Vec::new();
                res.extend_from_slice(&lines[..half]);
                res.push("...(中间省略)...");
                res.extend_from_slice(&lines[lines.len() - half..]);
                res
            }
        }
    }

    fn _save_full_output(
        &self,
        tool_name: &str,
        output: &str,
        metadata: Option<HashMap<String, Value>>,
    ) -> String {
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
            let json_str = serde_json::to_string_pretty(&data).unwrap_or_default();
            file.write_all(json_str.as_bytes()).ok();
        }

        filepath.to_string_lossy().to_string()
    }
}

// =============================================================================
// 测试用例
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_no_truncation_needed() {
        let truncator = ObservationTruncator::new(
            10,
            1000,
            TruncateDirection::Head,
            std::env::temp_dir().join("trunc_test"),
        );
        let result = truncator.truncate("echo", "short text", None);
        assert!(!result.truncated);
        assert_eq!(result.preview, "short text");
        assert!(result.full_output_path.is_none());
    }

    #[test]
    fn test_head_truncation() {
        let truncator = ObservationTruncator::new(
            3,
            1000,
            TruncateDirection::Head,
            std::env::temp_dir().join("trunc_test"),
        );
        let output = "line1\nline2\nline3\nline4\nline5";
        let result = truncator.truncate("test", output, None);
        assert!(result.truncated);
        let preview_lines: Vec<&str> = result.preview.lines().collect();
        assert_eq!(preview_lines.len(), 3);
        assert_eq!(preview_lines[0], "line1");
        assert_eq!(preview_lines[2], "line3");
    }

    #[test]
    fn test_tail_truncation() {
        let truncator = ObservationTruncator::new(
            3,
            1000,
            TruncateDirection::Tail,
            std::env::temp_dir().join("trunc_test"),
        );
        let output = "a\nb\nc\nd\ne";
        let result = truncator.truncate("test", output, None);
        assert!(result.truncated);
        let preview_lines: Vec<&str> = result.preview.lines().collect();
        assert_eq!(preview_lines.len(), 3);
        assert_eq!(preview_lines[0], "c");
    }

    #[test]
    fn test_head_tail_truncation() {
        let truncator = ObservationTruncator::new(
            4,
            1000,
            TruncateDirection::HeadTail,
            std::env::temp_dir().join("trunc_test"),
        );
        let output = "1\n2\n3\n4\n5\n6\n7\n8";
        let result = truncator.truncate("test", output, None);
        assert!(result.truncated);
        let preview = &result.preview;
        assert!(preview.contains("...(中间省略)..."));
        // 行：前 half=2 行，中间省略一行，后 half=2 行，总共5行
        let lines: Vec<&str> = preview.lines().collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "1");
        assert_eq!(lines[1], "2");
        assert_eq!(lines[2], "...(中间省略)...");
        assert_eq!(lines[3], "7");
        assert_eq!(lines[4], "8");
    }

    #[test]
    fn test_truncation_saves_full_output() {
        let dir = std::env::temp_dir().join("trunc_test2");
        let _ = fs::remove_dir_all(&dir);
        let truncator = ObservationTruncator::new(2, 100, TruncateDirection::Head, dir.clone());
        let output = "line1\nline2\nline3";
        let result = truncator.truncate("savetest", output, None);
        assert!(result.truncated);
        assert!(result.full_output_path.is_some());
        let path = result.full_output_path.as_ref().unwrap();
        let saved = fs::read_to_string(path).unwrap();
        assert!(saved.contains("line1"));
        let _ = fs::remove_dir_all(&dir);
    }
}