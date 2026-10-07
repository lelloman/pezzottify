#[path = "../src/logging.rs"]
mod logging;
use std::process::Command;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[test]
fn probe() {
    let Ok(mode) = std::env::var("LOGGING_PROBE") else {
        return;
    };
    let filter = if std::env::var("PROBE_ENTRY").unwrap() == "server" {
        EnvFilter::builder()
            .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
            .with_env_var("LOG_LEVEL")
            .from_env_lossy()
    } else {
        EnvFilter::from_default_env()
    };
    if mode == "legacy" {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    } else {
        logging::init(filter).unwrap();
    }
    let span = tracing::info_span!("PROBE_span", item = 7);
    let _guard = span.enter();
    tracing::error!(target: "probe", count = 3, enabled = true, "PROBE_error");
    tracing::info!(target: "probe", "PROBE_info");
    tracing::debug!(target: "probe", "PROBE_debug");
    tracing::trace!(target: "probe", "PROBE_trace");
    log::info!(target: "probe", "PROBE_legacy_log");
}

#[test]
fn logging_matches_previous_formatter_in_fresh_processes() {
    for filter in [
        None,
        Some(""),
        Some(" "),
        Some("off"),
        Some("info"),
        Some("debug"),
        Some("trace"),
        Some("warn,probe=debug"),
        Some("invalid["),
        Some("[PROBE_span{item=7}]=trace"),
    ] {
        for entry in ["server", "index"] {
            for color in [None, Some(""), Some("1")] {
                let run = |mode| {
                    let mut cmd = Command::new(std::env::current_exe().unwrap());
                    cmd.args(["--exact", "probe", "--nocapture"])
                        .env("LOGGING_PROBE", mode)
                        .env("PROBE_ENTRY", entry)
                        .env_remove("LOG_LEVEL")
                        .env_remove("RUST_LOG")
                        .env_remove("NO_COLOR");
                    if let Some(filter) = filter {
                        cmd.env("RUST_LOG", filter).env("LOG_LEVEL", filter);
                    }
                    if let Some(color) = color {
                        cmd.env("NO_COLOR", color);
                    }
                    let out = cmd.output().unwrap();
                    assert!(
                        out.status.success(),
                        "{}",
                        String::from_utf8_lossy(&out.stderr)
                    );
                    let stdout = String::from_utf8(out.stdout).unwrap();
                    assert!(
                        stdout.contains("1 passed"),
                        "probe did not execute: {stdout}"
                    );
                    let events: Vec<String> = stdout
                        .lines()
                        .filter(|line| line.contains("PROBE_"))
                        .map(|line| line.split_once('Z').expect("timestamp").1.to_owned())
                        .collect();
                    (events, out.stderr)
                };
                let old = run("legacy");
                let new = run("shared");
                assert_eq!(old, new, "filter={filter:?}, NO_COLOR={color:?}");
                if filter == Some("trace") {
                    assert_eq!(new.0.len(), 5);
                }
                if filter == Some("off") {
                    assert!(new.0.is_empty());
                }
            }
        }
    }
}
