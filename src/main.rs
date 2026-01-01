mod app;
mod config;
mod logging;
mod metrics;
mod proxy;
mod quic;
mod readers;
mod rewrite;
mod rewriters;
mod server;
mod sni;
mod tls_utils;
mod upstream;
mod utils;

use anyhow::{Context, Result};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize rustls crypto provider before any TLS operations
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|e| anyhow::anyhow!("Failed to install default crypto provider: {:?}", e))?;

    // Load config first (before logging init) to get logging config
    let config = config::AppConfig::load_or_default("config.toml");

    // Validate configuration before starting
    config
        .validate()
        .context("Configuration validation failed")?;

    // Initialize logging system
    let _guard =
        logging::init_logging(&config.logging).context("Failed to initialize logging system")?;

    info!("DNS Proxy Server starting...");
    info!(
        "Logging initialized - level: {}, file: {:?}, json: {}",
        config.logging.level, config.logging.file, config.logging.json
    );

    // Create and start app
    let mut app = app::App::new(config);
    app.start().context("Failed to start DNS Proxy Server")?;

    info!("DNS Proxy Server started successfully. Press Ctrl+C to shutdown.");

    // Wait for shutdown signal
    tokio::signal::ctrl_c()
        .await
        .context("Failed to listen for shutdown signal")?;

    info!("Shutdown signal received, shutting down gracefully...");
    app.wait_for_shutdown().await;

    Ok(())
}
