use anyhow::{Result, Context};
use hello_agents::tools::tool_registry::global_registry;
use hello_agents::tools::builtin::file_tools::{ReadTool, WriteTool, EditTool, MultiEditTool};

#[tokio::main]
async fn main() ->Result<()>{
    // 全局注册中心
    let guard = global_registry();
    let mut registry = guard.lock().await;
    registry.register_tool(Box::new(ReadTool::new("./", None, None)), false);
    registry.register_tool(Box::new(WriteTool::new("./", None, None)), false);
    registry.register_tool(Box::new(EditTool::new("./", None, None)), false);

    // 在注册完成后，执行tool
    // 具体测试见[file_tools中提供的测试用例](crates/hello_agents/src/tools/builtin/file_tools.rs)


    Ok(())
}