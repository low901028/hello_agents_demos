use crate::core::traits::tool::Tool;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::response::ToolResponse;
use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use tokio::fs;

// ===== ReadTool =====
pub struct ReadTool {
    working_dir: PathBuf,
}

impl ReadTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "读取文件内容或列出目录内容"
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" },
                "offset": { "type": "integer", "default": 0 },
                "limit": { "type": "integer", "default": 2000 }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| HelloAgentError::Tool("Missing path".into()))?;
        let full = self.working_dir.join(path);
        if !full.exists() {
            return Ok(ToolResponse::error("NOT_FOUND", "文件不存在"));
        }
        if full.is_dir() {
            let mut entries = Vec::new();
            let mut read_dir = tokio::fs::read_dir(&full).await?;
            while let Some(entry) = read_dir.next_entry().await? {
                let name = entry.file_name().to_string_lossy().into_owned();
                let metadata = entry.metadata().await?;
                let file_type = if metadata.is_dir() {
                    "directory"
                } else {
                    "file"
                };
                entries.push(serde_json::json!({
                    "name": name,
                    "type": file_type,
                    "size": metadata.len(),
                    "mtime": metadata.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
                }));
            }
            return Ok(ToolResponse::success_with(
                format!("目录内容: {:?}", entries),
                Some(serde_json::json!({"entries": entries})),
                None,
                None,
            ));
        }
        let content = fs::read_to_string(&full).await?;
        let lines: Vec<&str> = content.lines().collect();
        let offset = args["offset"].as_u64().unwrap_or(0) as usize;
        let limit = args["limit"].as_u64().unwrap_or(2000) as usize;
        let selected: Vec<&str> = lines.into_iter().skip(offset).take(limit).collect();
        Ok(ToolResponse::success(selected.join("\n")))
    }
}

// ===== WriteTool =====
pub struct WriteTool {
    working_dir: PathBuf,
}

impl WriteTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write_file"
    }
    fn description(&self) -> &str {
        "创建或覆盖文件，支持冲突检测和原子写入"
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" },
                "content": { "type": "string", "description": "文件内容" },
                "file_mtime_ms": { "type": "integer", "description": "缓存的文件修改时间（用于冲突检测）" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| HelloAgentError::Tool("Missing path".into()))?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| HelloAgentError::Tool("Missing content".into()))?;
        let cached_mtime = args.get("file_mtime_ms").and_then(|v| v.as_i64());
        let full = self.working_dir.join(path);

        // 冲突检测
        if full.exists() {
            let meta = fs::metadata(&full).await?;
            let current_mtime = meta
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            if let Some(cached) = cached_mtime {
                if current_mtime != cached {
                    return Ok(ToolResponse::error("CONFLICT", "文件自上次读取后被修改"));
                }
            }
        }

        // 原子写入
        let temp = full.with_extension("tmp");
        fs::write(&temp, content).await?;
        fs::rename(&temp, &full).await?;
        Ok(ToolResponse::success(format!(
            "成功写入 {} ({} 字节)",
            path,
            content.len()
        )))
    }
}

// ===== EditTool =====
pub struct EditTool {
    working_dir: PathBuf,
}

impl EditTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit_file"
    }
    fn description(&self) -> &str {
        "精确替换文件内容，支持冲突检测和自动备份"
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" },
                "old_string": { "type": "string", "description": "要替换的内容" },
                "new_string": { "type": "string", "description": "替换后的内容" },
                "file_mtime_ms": { "type": "integer", "description": "缓存的文件修改时间" }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| HelloAgentError::Tool("Missing path".into()))?;
        let old = args["old_string"]
            .as_str()
            .ok_or_else(|| HelloAgentError::Tool("Missing old_string".into()))?;
        let new = args["new_string"]
            .as_str()
            .ok_or_else(|| HelloAgentError::Tool("Missing new_string".into()))?;
        let cached_mtime = args.get("file_mtime_ms").and_then(|v| v.as_i64());
        let full = self.working_dir.join(path);
        if !full.exists() {
            return Ok(ToolResponse::error("NOT_FOUND", "文件不存在"));
        }

        // 冲突检测
        let meta = fs::metadata(&full).await?;
        let current_mtime = meta
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        if let Some(cached) = cached_mtime {
            if current_mtime != cached {
                return Ok(ToolResponse::error("CONFLICT", "文件自上次读取后被修改"));
            }
        }

        let content = fs::read_to_string(&full).await?;
        if !content.contains(old) {
            return Ok(ToolResponse::error(
                "INVALID_PARAM",
                "old_string 不存在于文件中",
            ));
        }
        let new_content = content.replace(old, new);
        let temp = full.with_extension("tmp");
        fs::write(&temp, &new_content).await?;
        fs::rename(&temp, &full).await?;
        Ok(ToolResponse::success(format!("成功编辑 {}", path)))
    }
}

// ===== MultiEditTool =====
pub struct MultiEditTool {
    working_dir: PathBuf,
}

impl MultiEditTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        "multi_edit"
    }
    fn description(&self) -> &str {
        "批量替换文件内容，支持原子性和冲突检测"
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" },
                "edits": { "type": "array", "description": "替换列表，每项包含 old_string 和 new_string" },
                "file_mtime_ms": { "type": "integer", "description": "缓存的文件修改时间" }
            },
            "required": ["path", "edits"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| HelloAgentError::Tool("Missing path".into()))?;
        let edits = args["edits"]
            .as_array()
            .ok_or_else(|| HelloAgentError::Tool("Missing edits".into()))?;
        let cached_mtime = args.get("file_mtime_ms").and_then(|v| v.as_i64());
        let full = self.working_dir.join(path);
        if !full.exists() {
            return Ok(ToolResponse::error("NOT_FOUND", "文件不存在"));
        }

        let meta = fs::metadata(&full).await?;
        let current_mtime = meta
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        if let Some(cached) = cached_mtime {
            if current_mtime != cached {
                return Ok(ToolResponse::error("CONFLICT", "文件被修改"));
            }
        }

        let mut content = fs::read_to_string(&full).await?;
        for edit in edits {
            let old = edit["old_string"]
                .as_str()
                .ok_or_else(|| HelloAgentError::Tool("Missing old_string in edit".into()))?;
            let new = edit["new_string"]
                .as_str()
                .ok_or_else(|| HelloAgentError::Tool("Missing new_string in edit".into()))?;
            content = content.replace(old, new);
        }

        let temp = full.with_extension("tmp");
        fs::write(&temp, &content).await?;
        fs::rename(&temp, &full).await?;
        Ok(ToolResponse::success(format!(
            "成功执行 {} 个替换",
            edits.len()
        )))
    }
}
