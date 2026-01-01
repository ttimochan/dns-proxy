use crate::config::AppConfig;
use crate::tls_utils;
use anyhow::{Context, Result};
use quinn::crypto::rustls::QuicServerConfig;
use quinn::{Endpoint, ServerConfig};
use std::net::SocketAddr;
use std::sync::Arc;

/// Create a QUIC server endpoint from application config
pub async fn create_quic_server_endpoint(
    config: &AppConfig,
    bind_addr: SocketAddr,
) -> Result<Endpoint> {
    // Create TLS server configuration
    let rustls_config = tls_utils::create_server_config(config)
        .await
        .context("Failed to create TLS server config")?;

    // rustls::ServerConfig is already compatible with quinn::rustls::ServerConfig
    let rustls_config_arc = Arc::new(rustls_config);
    let quic_server_config = QuicServerConfig::try_from(rustls_config_arc)
        .context("Failed to create QuicServerConfig")?;
    let quinn_server_config = ServerConfig::with_crypto(Arc::new(quic_server_config));

    Endpoint::server(quinn_server_config, bind_addr).context("Failed to create QUIC endpoint")
}
