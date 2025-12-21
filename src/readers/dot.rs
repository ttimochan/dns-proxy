use crate::config::AppConfig;
use crate::rewrite::SniRewriterType;
use crate::tls_utils;
use anyhow::{Context, Result};
use rustls::ServerName;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tracing::{error, info};

pub struct DoTServer {
    config: Arc<AppConfig>,
    rewriter: SniRewriterType,
}

impl DoTServer {
    pub fn new(config: Arc<AppConfig>, rewriter: SniRewriterType) -> Self {
        Self { config, rewriter }
    }

    pub async fn start(&self) -> Result<()> {
        let server_config = &self.config.servers.dot;
        if !server_config.enabled {
            info!("DoT server is disabled");
            return Ok(());
        }

        let server_tls_config = tls_utils::create_server_config(self.config.as_ref())
            .await
            .context("Failed to create TLS server config")?;
        let acceptor = TlsAcceptor::from(Arc::new(server_tls_config));

        let bind_addr = format!("{}:{}", server_config.bind_address, server_config.port);
        let listener = TcpListener::bind(&bind_addr)
            .await
            .with_context(|| format!("Failed to bind DoT server to {}", bind_addr))?;

        info!("DoT server listening on TCP {}", bind_addr);

        let upstream = self.config.dot_upstream();
        let rewriter = Arc::clone(&self.rewriter);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New DoT connection from {}", addr);
                    let acceptor = acceptor.clone();
                    let rewriter = Arc::clone(&rewriter);
                    let upstream_addr = upstream;
                    tokio::spawn(async move {
                        match acceptor.accept(stream).await {
                            Ok(tls_stream) => {
                                if let Err(e) =
                                    Self::handle_connection(tls_stream, rewriter, upstream_addr)
                                        .await
                                {
                                    error!("DoT connection handling error from {}: {}", addr, e);
                                } else {
                                    tracing::debug!(
                                        "DoT connection from {} completed successfully",
                                        addr
                                    );
                                }
                            }
                            Err(e) => {
                                error!("DoT TLS handshake error from {}: {}", addr, e);
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("DoT accept error on {}: {}", bind_addr, e);
                    // Add a small delay to prevent tight error loop
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
    }

    async fn handle_connection(
        stream: tokio_rustls::server::TlsStream<TcpStream>,
        _rewriter: SniRewriterType,
        upstream: std::net::SocketAddr,
    ) -> Result<()> {
        use anyhow::Context;
        use tracing::debug;

        let (mut reader, mut writer) = tokio::io::split(stream);

        // Read DNS message from client (zerocopy: use Bytes directly)
        let mut buffer = Vec::with_capacity(4096);
        reader
            .read_to_end(&mut buffer)
            .await
            .context("Failed to read DNS message from client")?;

        if buffer.is_empty() {
            debug!("Received empty DNS message, closing connection");
            return Ok(());
        }

        debug!(
            "Received DNS message: {} bytes, forwarding to upstream {}",
            buffer.len(),
            upstream
        );

        // Connect to upstream
        let upstream_stream = TcpStream::connect(upstream)
            .await
            .with_context(|| format!("Failed to connect to upstream DoT server: {}", upstream))?;

        let client_config =
            create_client_config().context("Failed to create TLS client configuration")?;
        let connector = TlsConnector::from(Arc::new(client_config));
        let sni_name = ServerName::try_from("dns.google")
            .context("Failed to create ServerName for upstream connection")?;

        let upstream_tls = connector
            .connect(sni_name, upstream_stream)
            .await
            .context("Failed to establish TLS connection to upstream")?;
        let (mut up_reader, mut up_writer) = tokio::io::split(upstream_tls);

        // Forward message (zerocopy: use slice reference)
        up_writer
            .write_all(&buffer)
            .await
            .context("Failed to write DNS message to upstream")?;
        up_writer
            .flush()
            .await
            .context("Failed to flush DNS message to upstream")?;

        // Read response (zerocopy: reuse buffer)
        buffer.clear();
        buffer.reserve(4096);
        up_reader
            .read_to_end(&mut buffer)
            .await
            .context("Failed to read DNS response from upstream")?;

        debug!(
            "Received DNS response: {} bytes, sending to client",
            buffer.len()
        );

        // Send response back (zerocopy: use slice reference)
        writer
            .write_all(&buffer)
            .await
            .context("Failed to write DNS response to client")?;
        writer
            .flush()
            .await
            .context("Failed to flush DNS response to client")?;

        Ok(())
    }
}

fn create_client_config() -> Result<rustls::ClientConfig> {
    Ok(rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth())
}
