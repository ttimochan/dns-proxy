use crate::config::AppConfig;
use crate::quic::create_quic_server_endpoint;
use crate::rewrite::SniRewriterType;
use crate::upstream::forward_quic_stream;
use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::Arc;
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

        let bind_addr = format!("{}:{}", server_config.bind_address, server_config.port);
        let addr: SocketAddr = bind_addr
            .parse()
            .with_context(|| format!("Invalid bind address: {}", bind_addr))?;

        let endpoint = create_quic_server_endpoint(self.config.as_ref(), addr).await?;
        info!("DoQ server listening on UDP {}", addr);

        let upstream = self.config.doq_upstream();
        let rewriter = Arc::clone(&self.rewriter);

        while let Some(conn) = endpoint.accept().await {
            let rewriter = Arc::clone(&rewriter);
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

        Ok(())
    }

    async fn handle_connection(
        connection: quinn::Connection,
        upstream: SocketAddr,
        _rewriter: SniRewriterType,
    ) -> Result<()> {
        loop {
            match connection.accept_bi().await {
                Ok((send, recv)) => {
                    // Forward stream using zerocopy where possible
                    if let Err(e) = forward_quic_stream(send, recv, upstream, "dns.google").await {
                        error!("DoQ stream forwarding error: {}", e);
                    }
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
}
