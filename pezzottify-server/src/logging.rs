//! Keep application logging policy while sharing subscriber installation.
use simple_server::logging::{self, AnsiMode, LogOutput, LoggingOptions};
use tracing_subscriber::EnvFilter;

pub fn init(filter: EnvFilter) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rendered = filter.to_string();
    let mut options = LoggingOptions::new(if rendered.is_empty() {
        "off"
    } else {
        &rendered
    });
    options.output = LogOutput::Stdout;
    options.ansi = if std::env::var("NO_COLOR").is_ok_and(|value| !value.is_empty()) {
        AnsiMode::Never
    } else {
        AnsiMode::Always
    };
    logging::try_init(options)?;
    // SubscriberInitExt previously installed this bridge implicitly.
    tracing_log::LogTracer::builder()
        .with_max_level(tracing_log::AsLog::as_log(
            &tracing::level_filters::LevelFilter::current(),
        ))
        .init()?;
    Ok(())
}
