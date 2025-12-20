use crate::config::AppConfig;
use crate::rewrite::SniRewriterType;
use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
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
        let socket = UdpSocket::bind(&bind_addr)
            .await
            .with_context(|| format!("Failed to bind DoQ server to {}", bind_addr))?;

        let socket = Arc::new(socket);
        info!("DoQ server listening on UDP {}", bind_addr);

        let upstream = self.config.doq_upstream();

        loop {
            let mut buf = vec![0u8; 4096];
            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    info!("DoQ packet from {}: {} bytes", addr, len);
                    let packet = buf[..len].to_vec();
                    let socket = Arc::clone(&socket);
                    let rewriter = Arc::clone(&self.rewriter);

                    tokio::spawn(async move {
                        if let Err(e) =
                            Self::handle_packet(socket, packet, addr, upstream, rewriter).await
                        {
                            error!("DoQ packet handling error from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("DoQ recv error: {}", e);
                }
            }
        }
    }

    async fn handle_packet(
        socket: Arc<UdpSocket>,
        packet: Vec<u8>,
        _from: SocketAddr,
        upstream: SocketAddr,
        _rewriter: SniRewriterType,
    ) -> Result<()> {
        socket.send_to(&packet, upstream).await?;
        Ok(())
    }
}
