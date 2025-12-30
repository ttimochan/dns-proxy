use crate::config::AppConfig;
use crate::metrics::Metrics;
use crate::proxy::handle_http_request;
use crate::rewrite::SniRewriterType;
use crate::upstream::create_http_client;
use crate::utils::BackoffCounter;
use anyhow::{Context, Result};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

pub struct DoHServer {
    config: Arc<AppConfig>,
    rewriter: SniRewriterType,
    client: crate::upstream::HttpClient,
    backoff: Arc<BackoffCounter>,
    metrics: Arc<Metrics>,
}

impl DoHServer {
    pub fn new(config: Arc<AppConfig>, rewriter: SniRewriterType, metrics: Arc<Metrics>) -> Self {
        Self {
            config,
            rewriter,
            client: create_http_client(),
            backoff: Arc::new(BackoffCounter::new()),
            metrics,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let server_config = &self.config.servers.doh;
        if !server_config.enabled {
            info!("DoH server is disabled");
            return Ok(());
        }

        let bind_addr = format!("{}:{}", server_config.bind_address, server_config.port);
        let listener = TcpListener::bind(&bind_addr)
            .await
            .with_context(|| format!("Failed to bind DoH server to {}", bind_addr))?;

        info!("DoH server listening on TCP {}", bind_addr);

        let rewriter = Arc::clone(&self.rewriter);
        let client = Arc::new(self.client.clone());
        let metrics = Arc::clone(&self.metrics);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let rewriter = Arc::clone(&rewriter);
                    let client = Arc::clone(&client);
                    let metrics = Arc::clone(&metrics);
                    tokio::spawn(async move {
                        let io = TokioIo::new(stream);
                        let service = service_fn(move |req| {
                            let rewriter = Arc::clone(&rewriter);
                            let client = Arc::clone(&client);
                            let metrics = Arc::clone(&metrics);
                            let client_addr = addr;
                            async move {
                                handle_http_request(req, rewriter, &client, metrics)
                                    .await
                                    .map_err(|e| {
                                        error!("DoH handler error from {}: {}", client_addr, e);
                                        std::io::Error::other(e.to_string())
                                    })
                            }
                        });

                        if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                            error!("DoH connection error from {}: {}", addr, e);
                        } else {
                            tracing::debug!("DoH connection from {} completed", addr);
                        }
                    });
                }
                Err(e) => {
                    error!("DoH accept error on {}: {}", bind_addr, e);
                    // Use exponential backoff to prevent tight error loop
                    let delay = self.backoff.next_delay(100, 5000);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}
