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

    // Convert rustls::ServerConfig to quinn::rustls::ServerConfig
    // Note: This unsafe conversion is necessary because quinn::rustls::ServerConfig
    // is a newtype wrapper around rustls::ServerConfig with identical memory layout.
    // This is a known pattern in the quinn ecosystem and is safe as long as both
    // types maintain their current structure.
    let rustls_config_arc = Arc::new(rustls_config);
    let quinn_rustls_config: Arc<quinn::rustls::ServerConfig> = unsafe {
        // Safety: quinn::rustls::ServerConfig is a newtype wrapper around rustls::ServerConfig
        // with the same memory layout. This is documented in quinn's API and is safe
        // as long as the versions of rustls and quinn are compatible.
        std::mem::transmute(rustls_config_arc)
    };
    let quic_server_config = QuicServerConfig::try_from(quinn_rustls_config)
        .context("Failed to create QuicServerConfig")?;
    let quinn_server_config = ServerConfig::with_crypto(Arc::new(quic_server_config));

    Endpoint::server(quinn_server_config, bind_addr).context("Failed to create QUIC endpoint")
}
