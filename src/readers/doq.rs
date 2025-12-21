use crate::config::AppConfig;
use crate::rewrite::SniRewriterType;
use crate::tls_utils;
use anyhow::{Context, Result};
use bytes::BytesMut;
use quinn::crypto::rustls::QuicServerConfig;
use quinn::{Endpoint, ServerConfig};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tracing::{error, info};

pub struct DoQServer {
    config: Arc<AppConfig>,
    rewriter: SniRewriterType,
}

impl DoQServer {
    pub fn new(config: Arc<AppConfig>, rewriter: SniRewriterType) -> Self {
        Self { config, rewriter }
    }

    pub async fn start(&self) -> Result<()> {
        let server_config = &self.config.servers.doq;
        if !server_config.enabled {
            info!("DoQ server is disabled");
            return Ok(());
        }

        // Create QUIC server configuration
        let rustls_config = tls_utils::create_server_config(self.config.as_ref())
            .await
            .context("Failed to create TLS server config for DoQ")?;

        // Convert rustls::ServerConfig to quinn::rustls::ServerConfig
        // quinn::rustls::ServerConfig is a type alias, so we can use it directly
        let rustls_config_arc = Arc::new(rustls_config);
        // We need to use quinn's rustls wrapper
        let quinn_rustls_config: Arc<quinn::rustls::ServerConfig> = unsafe {
            // Safety: quinn::rustls::ServerConfig is a newtype wrapper around rustls::ServerConfig
            // with the same memory layout
            std::mem::transmute(rustls_config_arc)
        };
        let quic_server_config = QuicServerConfig::try_from(quinn_rustls_config)
            .context("Failed to create QuicServerConfig")?;
        let quinn_server_config = ServerConfig::with_crypto(Arc::new(quic_server_config));

        let bind_addr = format!("{}:{}", server_config.bind_address, server_config.port);
        let addr: SocketAddr = bind_addr
            .parse()
            .with_context(|| format!("Invalid bind address: {}", bind_addr))?;

        let endpoint = Endpoint::server(quinn_server_config, addr)
            .context("Failed to create QUIC endpoint")?;

        info!("DoQ server listening on UDP {}", addr);

        let upstream = self.config.doq_upstream();
        let rewriter = Arc::clone(&self.rewriter);

        loop {
            match endpoint.accept().await {
                Some(conn) => {
                    let rewriter = Arc::clone(&rewriter);
                    let upstream = upstream;
                    tokio::spawn(async move {
                        match conn.await {
                            Ok(connection) => {
                                info!("New DoQ connection from {}", connection.remote_address());
                                if let Err(e) =
                                    Self::handle_connection(connection, upstream, rewriter).await
                                {
                                    error!("DoQ connection error: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("DoQ connection error: {}", e);
                            }
                        }
                    });
                }
                None => {
                    // Endpoint closed
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_connection(
        connection: quinn::Connection,
        upstream: SocketAddr,
        _rewriter: SniRewriterType,
    ) -> Result<()> {
        loop {
            match connection.accept_bi().await {
                Ok((mut send, mut recv)) => {
                    // Read DNS message from client
                    let mut buffer = BytesMut::with_capacity(4096);
                    recv.read_buf(&mut buffer).await?;

                    if buffer.is_empty() {
                        continue;
                    }

                    info!("DoQ: received {} bytes from client", buffer.len());

                    // Forward to upstream QUIC server
                    let upstream_conn = Self::connect_upstream(upstream).await?;
                    let (mut up_send, mut up_recv) = upstream_conn.open_bi().await?;

                    // Send DNS message to upstream
                    up_send.write_all(&buffer).await?;
                    up_send
                        .finish()
                        .map_err(|e| anyhow::anyhow!("Failed to finish upstream stream: {}", e))?;

                    // Read response from upstream
                    let mut response = BytesMut::with_capacity(4096);
                    up_recv.read_buf(&mut response).await?;

                    // Send response back to client
                    send.write_all(&response).await?;
                    send.finish()
                        .map_err(|e| anyhow::anyhow!("Failed to finish client stream: {}", e))?;
                }
                Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
                    info!("DoQ connection closed");
                    break;
                }
                Err(e) => {
                    error!("DoQ stream error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    async fn connect_upstream(addr: SocketAddr) -> Result<quinn::Connection> {
        use quinn::crypto::rustls::QuicClientConfig;
        use quinn::rustls::pki_types::CertificateDer;
        use quinn::rustls::{ClientConfig, RootCertStore};

        // Create client TLS config
        let mut root_store = RootCertStore::empty();
        for cert in rustls_native_certs::load_native_certs()? {
            root_store.add(CertificateDer::from(cert.0))?;
        }

        let mut client_crypto = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        let quic_client_config = QuicClientConfig::try_from(client_crypto)
            .context("Failed to create QuicClientConfig")?;
        let client_config = quinn::ClientConfig::new(Arc::new(quic_client_config));

        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        endpoint.set_default_client_config(client_config);

        let connection = endpoint
            .connect(addr, "dns.google")?
            .await
            .context("Failed to connect to upstream DoQ server")?;

        Ok(connection)
    }
}
