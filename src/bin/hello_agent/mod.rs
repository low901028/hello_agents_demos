pub mod agents;
pub mod context;
pub mod core;
pub mod observability;
pub mod skills;
pub mod tools;

pub mod examples;

// 配置第三方库的日志级别
pub fn init_logging() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        //.filter_module("reqwest", log::LevelFilter::Warn)
        //.filter_module("hyper", log::LevelFilter::Warn)
        .init();
}
