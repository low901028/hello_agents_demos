mod hello_agent;
use anyhow::{Result,Context};

fn main() -> Result<()>{
    crate::hello_agent::examples::tool_response_demo::main()
}