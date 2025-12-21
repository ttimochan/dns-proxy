mod app;
mod config;
mod proxy;
mod quic;
mod readers;
mod rewrite;
mod rewriters;
mod sni;
mod tls_utils;
mod upstream;

use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = config::AppConfig::load_or_default("config.toml");
    info!("Configuration loaded");

    let app = app::App::new(config);
    app.start()?;

    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

    Ok(())
}
