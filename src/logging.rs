use crate::config::LoggingConfig;
use anyhow::{Context, Result};
use std::str::FromStr;
use tracing_subscriber::fmt::time::ChronoUtc;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Initialize logging system based on configuration
pub fn init_logging(
    config: &LoggingConfig,
) -> Result<Option<tracing_appender::non_blocking::WorkerGuard>> {
    // Parse log level from config or environment variable
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| config.level.clone());

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::from_str(&log_level).unwrap_or_else(|_| EnvFilter::new("info"))
    });

    let mut guard: Option<tracing_appender::non_blocking::WorkerGuard> = None;

    if let Some(log_file) = &config.file {
        // File logging with rotation
        if config.rotation {
            let file_appender = tracing_appender::rolling::daily(
                std::path::Path::new(log_file)
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(".")),
                std::path::Path::new(log_file)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("dns-proxy.log"),
            );

            let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);
            guard = Some(file_guard);

            if config.json {
                // JSON format for file
                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_timer(ChronoUtc::rfc_3339())
                    .json()
                    .with_filter(env_filter.clone());

                // Console logging (non-JSON)
                let console_layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_timer(ChronoUtc::rfc_3339())
                    .with_filter(env_filter);

                tracing_subscriber::registry()
                    .with(file_layer)
                    .with(console_layer)
                    .init();
            } else {
                // Plain text format for file
                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_timer(ChronoUtc::rfc_3339())
                    .with_filter(env_filter.clone());

                // Console logging
                let console_layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_timer(ChronoUtc::rfc_3339())
                    .with_filter(env_filter);

                tracing_subscriber::registry()
                    .with(file_layer)
                    .with(console_layer)
                    .init();
            }
        } else {
            // Simple file logging without rotation
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file)
                .with_context(|| format!("Failed to open log file: {}", log_file))?;

            if config.json {
                // JSON format
                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(file)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_timer(ChronoUtc::rfc_3339())
                    .json()
                    .with_filter(env_filter.clone());

                // Console logging (non-JSON)
                let console_layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_timer(ChronoUtc::rfc_3339())
                    .with_filter(env_filter);

                tracing_subscriber::registry()
                    .with(file_layer)
                    .with(console_layer)
                    .init();
            } else {
                // Plain text format
                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(file)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_timer(ChronoUtc::rfc_3339())
                    .with_filter(env_filter.clone());

                // Console logging
                let console_layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_timer(ChronoUtc::rfc_3339())
                    .with_filter(env_filter);

                tracing_subscriber::registry()
                    .with(file_layer)
                    .with(console_layer)
                    .init();
            }
        }
    } else {
        // Console logging only
        if config.json {
            tracing_subscriber::fmt()
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
                .with_timer(ChronoUtc::rfc_3339())
                .json()
                .with_env_filter(env_filter)
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
                .with_timer(ChronoUtc::rfc_3339())
                .with_env_filter(env_filter)
                .init();
        }
    }

    Ok(guard)
}
