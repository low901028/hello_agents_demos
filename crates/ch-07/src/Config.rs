/// =========================================
/// 配置类： LLM配置和系统配置
/// =========================================
#[derive(Clone)]
pub struct Config {
    pub model: String,
    pub provider: String,
    pub temperature: f32,
    pub max_tokens: Option<usize>,
    pub debug: bool,
    pub log_level: String,
    pub max_history_length: usize,
}

impl Config {
    pub fn new(model: Option<String>,
               provider: Option<String>,
               temperature: Option<f32>,
               max_tokens: Option<usize>,
               debug: Option<bool>,
               log_level: Option<String>,
               max_history_length: Option<usize>) -> Self {
        Config {
            model: model.unwrap_or_else(||{  "deepseek-v4-flash".to_string()  }),
            provider: provider.unwrap_or_else(||{ "deepseek".to_string() }),
            temperature: temperature.unwrap_or_else(||{0.7}),
            max_tokens: max_tokens.or_else(||{None}),
            debug: debug.unwrap_or_else(||{false}),
            log_level: log_level.unwrap_or_else(||{"INFO".to_string()}),
            max_history_length: max_history_length.unwrap_or_else(||{100}),
        }

    }
}