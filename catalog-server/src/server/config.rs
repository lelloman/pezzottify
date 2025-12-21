use super::RequestsLoggingLevel;

#[derive(Clone)]
pub struct ServerConfig {
    pub requests_logging_level: RequestsLoggingLevel,
    pub port: u16,
    pub content_cache_age_sec: usize,
    pub frontend_dir_path: Option<String>,
    /// If true, disables the password authentication endpoint.
    /// Users must authenticate via OIDC only.
    pub disable_password_auth: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            requests_logging_level: RequestsLoggingLevel::Path,
            port: 3001,
            content_cache_age_sec: 3600,
            frontend_dir_path: None,
            disable_password_auth: false,
        }
    }
}
