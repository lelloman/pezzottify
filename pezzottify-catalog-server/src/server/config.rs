use super::RequestsLoggingLevel;

#[derive(Clone)]
pub struct ServerConfig {
    pub requests_logging_level: RequestsLoggingLevel,
    pub port: u16,
    pub content_cache_age_sec: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            requests_logging_level: RequestsLoggingLevel::Path,
            port: 3001,
            content_cache_age_sec: 3600,
        }
    }
}
