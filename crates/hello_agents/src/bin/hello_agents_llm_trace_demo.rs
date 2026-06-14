use hello_agents::infra::trace::trace_logger::TraceLogger;

fn main() {
    let mut trace_logger = TraceLogger::new("memory/traces", false, Some(true));
    // let trace_logger = TraceLogger::new("memory/traces", false, None);
    // let trace_logger = TraceLogger::new("memory/traces", true, Some(true));
    trace_logger.log_event("session_start", serde_json::json!({"agent_name": "MyAgent"}), None);
    trace_logger.log_event("tool_call", serde_json::json!({"tool_name": "Calculator"}), Some(1));
    trace_logger.finalize();  // 生成最终 HTML
}