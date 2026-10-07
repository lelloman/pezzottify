//! Preserve the application's environment, output and ANSI policy in the engine.
use simple_server::engine_logging::{self, AnsiMode, FilterMode, LogOutput, LoggingOptions};
pub fn init_from_env(
    variable: &str,
    default_info: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut options = LoggingOptions::new(std::env::var(variable).unwrap_or_default());
    options.output = LogOutput::Stdout;
    options.ansi = if std::env::var("NO_COLOR").is_ok_and(|v| !v.is_empty()) {
        AnsiMode::Never
    } else {
        AnsiMode::Always
    };
    engine_logging::try_init(
        options,
        if default_info {
            FilterMode::LossyOrInfo
        } else {
            FilterMode::Lossy
        },
    )?;
    engine_logging::init_log_bridge()?;
    Ok(())
}
