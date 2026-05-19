pub mod base;
pub mod openai;
pub mod anthropic;
pub mod gemini;

use base::BaseLLMAdapter;
use openai::OpenAIAdapter;
use anthropic::AnthropicAdapter;
use gemini::GeminiAdapter;

/// 根据 base_url 自动选择适配器
pub fn create_adapter(
    api_key: String,
    base_url: Option<String>,
    timeout: u64,
    model: String,
) -> Box<dyn BaseLLMAdapter> {
    if let Some(ref url) = base_url {
        let url_lower = url.to_lowercase();

        if url_lower.contains("anthropic.com") {
            return Box::new(AnthropicAdapter::new(api_key, url.clone(), model, timeout));
        }

        if url_lower.contains("googleapis.com") || url_lower.contains("generativelanguage") {
            return Box::new(GeminiAdapter::new(api_key, url.clone(), model, timeout));
        }
    }

    Box::new(OpenAIAdapter::new(
        api_key,
        base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
        model,
        timeout,
    ))
}