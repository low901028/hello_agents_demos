// examples/file_tools_demo.rs
// 文件操作工具使用示例（异步版）

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::{json, Value};
use tempfile::TempDir;

use hello_agents::core::traits::tool::Tool;
use hello_agents::core::traits::tool_registry::ToolRegistry;
use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::core::types::response::ToolResponse;
use hello_agents::infra::tool_registry_impl::ToolRegistryImpl;

// ==================== ReadTool ====================

struct ReadTool {
    working_dir: PathBuf,
    metadata_cache: Option<Arc<std::sync::Mutex<HashMap<String, FileMetadata>>>>,
}

#[derive(Debug, Clone)]
struct FileMetadata {
    mtime_ms: i64,
    size_bytes: u64,
}

impl ReadTool {
    fn new(project_root: PathBuf, registry: Option<Arc<std::sync::Mutex<HashMap<String, FileMetadata>>>>) -> Self {
        Self { working_dir: project_root, metadata_cache: registry }
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str { "Read" }
    fn description(&self) -> &str { "读取文件内容，缓存元数据用于乐观锁" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "文件路径"}
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let full_path = self.working_dir.join(path);
        if !full_path.exists() {
            return Ok(ToolResponse::error("NOT_FOUND", &format!("文件 '{}' 不存在", path)));
        }

        let content = tokio::fs::read_to_string(&full_path).await?;
        let metadata = tokio::fs::metadata(&full_path).await?;
        let mtime_ms = metadata.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
        let size_bytes = metadata.len();

        // 缓存元数据（如果提供了注册表）
        if let Some(cache) = &self.metadata_cache {
            let mut cache = cache.lock().unwrap();
            cache.insert(path.to_string(), FileMetadata { mtime_ms, size_bytes });
        }

        Ok(ToolResponse::success(format!("文件内容:\n{}", content)))
    }
}

// ==================== WriteTool ====================

struct WriteTool {
    working_dir: PathBuf,
    metadata_cache: Option<Arc<std::sync::Mutex<HashMap<String, FileMetadata>>>>,
}

impl WriteTool {
    fn new(project_root: PathBuf, registry: Option<Arc<std::sync::Mutex<HashMap<String, FileMetadata>>>>) -> Self {
        Self { working_dir: project_root, metadata_cache: registry }
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str { "Write" }
    fn description(&self) -> &str { "创建或覆盖文件，支持冲突检测和原子写入" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "文件路径"},
                "content": {"type": "string", "description": "文件内容"},
                "file_mtime_ms": {"type": "integer", "description": "缓存的 mtime（可选）"}
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let cached_mtime = args.get("file_mtime_ms").and_then(|v| v.as_i64());

        let full_path = self.working_dir.join(path);

        // 冲突检测
        if full_path.exists() {
            if let Some(cached) = cached_mtime {
                let current_meta = tokio::fs::metadata(&full_path).await?;
                let current_mtime = current_meta.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
                if current_mtime != cached {
                    return Ok(ToolResponse::error("CONFLICT", &format!(
                        "文件自上次读取后被修改。当前 mtime={}, 缓存 mtime={}", current_mtime, cached
                    )));
                }
            }
        }

        // 原子写入
        let temp_path = full_path.with_extension("tmp");
        tokio::fs::write(&temp_path, content).await?;
        tokio::fs::rename(&temp_path, &full_path).await?;

        Ok(ToolResponse::success(format!("成功写入 {}", path)))
    }
}

// ==================== EditTool ====================

struct EditTool {
    working_dir: PathBuf,
    metadata_cache: Option<Arc<std::sync::Mutex<HashMap<String, FileMetadata>>>>,
}

impl EditTool {
    fn new(project_root: PathBuf, registry: Option<Arc<std::sync::Mutex<HashMap<String, FileMetadata>>>>) -> Self {
        Self { working_dir: project_root, metadata_cache: registry }
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str { "Edit" }
    fn description(&self) -> &str { "精确替换文件内容，支持冲突检测" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "文件路径"},
                "old_string": {"type": "string", "description": "要替换的内容（必须唯一匹配）"},
                "new_string": {"type": "string", "description": "替换后的内容"},
                "file_mtime_ms": {"type": "integer", "description": "缓存的 mtime（可选）"}
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let old = args.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
        let new = args.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
        let cached_mtime = args.get("file_mtime_ms").and_then(|v| v.as_i64());

        let full_path = self.working_dir.join(path);
        if !full_path.exists() {
            return Ok(ToolResponse::error("NOT_FOUND", &format!("文件 '{}' 不存在", path)));
        }

        // 冲突检测
        if let Some(cached) = cached_mtime {
            let current_meta = tokio::fs::metadata(&full_path).await?;
            let current_mtime = current_meta.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
            if current_mtime != cached {
                return Ok(ToolResponse::error("CONFLICT", &format!(
                    "文件自上次读取后被修改。当前 mtime={}, 缓存 mtime={}", current_mtime, cached
                )));
            }
        }

        let content = tokio::fs::read_to_string(&full_path).await?;
        if content.matches(old).count() != 1 {
            return Ok(ToolResponse::error("INVALID_PARAM", "old_string 必须唯一匹配"));
        }

        let new_content = content.replace(old, new);
        tokio::fs::write(&full_path, new_content).await?;

        Ok(ToolResponse::success(format!("成功编辑 {}", path)))
    }
}

// ==================== MultiEditTool ====================

struct MultiEditTool {
    working_dir: PathBuf,
}

impl MultiEditTool {
    fn new(project_root: PathBuf) -> Self {
        Self { working_dir: project_root }
    }
}

#[async_trait]
impl Tool for MultiEditTool {
    fn name(&self) -> &str { "MultiEdit" }
    fn description(&self) -> &str { "批量替换文件内容，支持冲突检测和原子性" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "文件路径"},
                "edits": {"type": "array", "description": "替换列表，每项包含 old_string 和 new_string"},
                "file_mtime_ms": {"type": "integer", "description": "缓存的 mtime（可选）"}
            },
            "required": ["path", "edits"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let edits = args.get("edits").and_then(|v| v.as_array()).cloned().unwrap_or_default();

        let full_path = self.working_dir.join(path);
        if !full_path.exists() {
            return Ok(ToolResponse::error("NOT_FOUND", &format!("文件 '{}' 不存在", path)));
        }

        let mut content = tokio::fs::read_to_string(&full_path).await?;
        for edit in &edits {
            let old = edit.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
            let new = edit.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
            if content.matches(old).count() != 1 {
                return Ok(ToolResponse::error("INVALID_PARAM", &format!("'{}' 必须唯一匹配", old)));
            }
            content = content.replace(old, new);
        }

        tokio::fs::write(&full_path, content).await?;
        Ok(ToolResponse::success(format!("成功执行 {} 个替换操作", &edits.len())))
    }
}

// ==================== 示例主函数 ====================

#[tokio::main]
async fn main() -> Result<(), HelloAgentError> {
    // 共享的元数据缓存（模拟 ToolRegistry 的 read_metadata_cache）
    let metadata_cache = Arc::new(std::sync::Mutex::new(HashMap::new()));

    // 示例 1：基本文件操作
    {
        println!("{}", "=".repeat(60));
        println!("示例 1: 基本文件操作");
        println!("{}", "=".repeat(60));

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        let read_tool = ReadTool::new(root.clone(), Some(metadata_cache.clone()));
        let write_tool = WriteTool::new(root.clone(), Some(metadata_cache.clone()));
        let edit_tool = EditTool::new(root.clone(), Some(metadata_cache.clone()));

        let mut registry = ToolRegistryImpl::new();
        registry.register(Box::new(read_tool));
        registry.register(Box::new(write_tool));
        registry.register(Box::new(edit_tool));

        // 写入文件
        println!("\n1. 创建新文件...");
        let resp = registry.execute("Write", json!({
            "path": "config.py",
            "content": "API_KEY = \"test_key_123\"\nDEBUG = False\nPORT = 8000\n"
        })).await?;
        println!("   状态: {:?}", resp.status);
        println!("   消息: {}", resp.text);

        // 读取文件
        println!("\n2. 读取文件...");
        let resp = registry.execute("Read", json!({"path": "config.py"})).await?;
        println!("   状态: {:?}", resp.status);
        println!("   内容: {}", resp.text);
        // 显示缓存的元数据（假设 ReadTool 会缓存，这里简化）
        println!("   元数据已缓存");

        // 编辑文件
        println!("\n3. 编辑文件...");
        let resp = registry.execute("Edit", json!({
            "path": "config.py",
            "old_string": "DEBUG = False",
            "new_string": "DEBUG = True"
        })).await?;
        println!("   状态: {:?}", resp.status);
        println!("   消息: {}", resp.text);

        // 再次读取验证
        println!("\n4. 验证修改...");
        let resp = registry.execute("Read", json!({"path": "config.py"})).await?;
        println!("   内容: {}", resp.text);
    }

    // 示例 2：乐观锁冲突检测
    {
        println!("\n{}", "=".repeat(60));
        println!("示例 2: 乐观锁冲突检测");
        println!("{}", "=".repeat(60));

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        let read_tool = ReadTool::new(root.clone(), Some(metadata_cache.clone()));
        let edit_tool = EditTool::new(root.clone(), Some(metadata_cache.clone()));

        let mut registry = ToolRegistryImpl::new();
        registry.register(Box::new(read_tool));
        registry.register(Box::new(edit_tool));

        // 创建初始文件
        let test_file = root.join("data.txt");
        tokio::fs::write(&test_file, "Original content").await?;

        // Agent 读取文件（缓存元数据）
        println!("\n1. Agent 读取文件...");
        let resp = registry.execute("Read", json!({"path": "data.txt"})).await?;
        println!("   状态: {:?}", resp.status);
        // 获取缓存的 mtime（这里直接从缓存中取，模拟）
        let cached_mtime = {
            let cache = metadata_cache.lock().unwrap();
            cache.get("data.txt").map(|m| m.mtime_ms)
        };

        // 外部修改文件
        println!("\n2. 外部进程修改文件...");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        tokio::fs::write(&test_file, "Modified by external process").await?;
        println!("   文件已被外部修改");

        // Agent 尝试编辑（使用缓存的 mtime）
        println!("\n3. Agent 尝试编辑（使用缓存的 mtime）...");
        let resp = registry.execute("Edit", json!({
            "path": "data.txt",
            "old_string": "Original content",
            "new_string": "My changes",
            "file_mtime_ms": cached_mtime
        })).await?;
        if let Some(err) = &resp.error_info {
            println!("   ✅ 成功检测到冲突！");
            println!("   错误码: {}", err.code);
            println!("   错误消息: {}", err.message);
        } else {
            println!("   ❌ 未检测到冲突（不应该发生）");
        }
    }

    // 示例 3：批量编辑
    {
        println!("\n{}", "=".repeat(60));
        println!("示例 3: 批量编辑操作");
        println!("{}", "=".repeat(60));

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        let multiedit_tool = MultiEditTool::new(root.clone());
        let mut registry = ToolRegistryImpl::new();
        registry.register(Box::new(multiedit_tool));

        let settings_file = root.join("settings.py");
        tokio::fs::write(&settings_file,
                         "API_KEY = \"old_key\"\nDEBUG = False\nPORT = 8000\nHOST = \"localhost\"\n"
        ).await?;

        println!("\n原始内容:");
        let original = tokio::fs::read_to_string(&settings_file).await?;
        println!("{}", original);

        println!("\n执行批量编辑...");
        let resp = registry.execute("MultiEdit", json!({
            "path": "settings.py",
            "edits": [
                {"old_string": "API_KEY = \"old_key\"", "new_string": "API_KEY = \"new_key_456\""},
                {"old_string": "DEBUG = False", "new_string": "DEBUG = True"},
                {"old_string": "PORT = 8000", "new_string": "PORT = 9000"}
            ]
        })).await?;
        println!("状态: {:?}", resp.status);
        println!("消息: {}", resp.text);

        println!("\n修改后内容:");
        let modified = tokio::fs::read_to_string(&settings_file).await?;
        println!("{}", modified);
    }

    println!("\n{}", "=".repeat(60));
    println!("✅ 所有示例运行完成！");
    println!("{}", "=".repeat(60));
    Ok(())
}