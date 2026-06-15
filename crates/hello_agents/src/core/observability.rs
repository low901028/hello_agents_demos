use serde_json::Value;

pub trait TraceLogger: Send + Sync {
    fn log_event(&self, event: &str, payload: Value, step: Option<usize>);
    fn finalize(&self);
    fn session_id(&self) -> &str;
}
