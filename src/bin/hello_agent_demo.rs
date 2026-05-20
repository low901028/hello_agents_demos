mod hello_agent;
use anyhow::{Context, Result};
// use hello_agent::examples::tools::{todowrite_real_world};
// use hello_agent::examples::custom_tools::{advanced_tool_template};
// use crate::hello_agent::examples::custom_tools::{code_formatter, custom_complate};
// use hello_agent::examples::custom_tools::expandable_tool_template;
// use crate::hello_agent::examples::subagencrate::hello_agentt_demo;

fn main() -> Result<()>{
    // tool_response_demo::main()
    //demo_todowrite::main()
    // todowrite_real_world::main()
    // advanced_tool_template::main()
    // code_formatter::main()
    // custom_complate::main()
    // expandable_tool_template::main()
    // subagent_demo::main()
    crate::hello_agent::init_logging();
    crate::hello_agent::examples::subagent_demo::main()

    //Ok(())
}