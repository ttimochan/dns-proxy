use crate::config::AppConfig;
use crate::metrics::{Metrics, Timer};
use crate::quic::create_quic_server_endpoint;
use crate::rewrite::SniRewriterType;
use crate::sni::SniRewriter;
use crate::upstream::{HttpClient, create_http_client, forward_http_request};
use anyhow::{Context, Result};
use bytes::{Buf, Bytes};
use h3::server::Connection as H3ServerConnection;
use hyper::Method;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, error, info};

pub struct DoH3Server {
    config: Arc<AppConfig>,
    rewriter: SniRewriterType,
    client: HttpClient,
    metrics: Arc<Metrics>,
}

impl DoH3Server {
    pub fn new(config: Arc<AppConfig>, rewriter: SniRewriterType, metrics: Arc<Metrics>) -> Self {
        Self {
            config,
            rewriter,
            client: create_http_client(),
            metrics,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let server_config = &self.config.servers.doh3;
        if !server_config.enabled {
            info!("DoH3 server is disabled");
            return Ok(());
        }

        let bind_addr = format!("{}:{}", server_config.bind_address, server_config.port);
        let addr: SocketAddr = bind_addr
            .parse()
            .with_context(|| format!("Invalid bind address: {}", bind_addr))?;

        let endpoint = create_quic_server_endpoint(self.config.as_ref(), addr).await?;
        info!("DoH3 server listening on UDP {}", addr);

        let rewriter = Arc::clone(&self.rewriter);
        let client = Arc::new(self.client.clone());
        let metrics = Arc::clone(&self.metrics);

        while let Some(conn) = endpoint.accept().await {
            let rewriter = Arc::clone(&rewriter);
            let client = Arc::clone(&client);
            let metrics = Arc::clone(&metrics);
            tokio::spawn(async move {
                match conn.await {
                    Ok(connection) => {
                        let remote_addr = connection.remote_address();
                        info!("New DoH3 connection from {}", remote_addr);
                        let metrics_clone = Arc::clone(&metrics);
                        if let Err(e) =
                            Self::handle_connection(connection, rewriter, &client, metrics).await
                        {
                            error!("DoH3 connection handling error from {}: {}", remote_addr, e);
                            metrics_clone.record_upstream_error();
                        } else {
                            debug!(
                                "DoH3 connection from {} completed successfully",
                                remote_addr
                            );
                        }
                    }
                    Err(e) => {
                        error!("DoH3 connection establishment error: {}", e);
                    }
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(
        connection: quinn::Connection,
        rewriter: SniRewriterType,
        client: &HttpClient,
        metrics: Arc<Metrics>,
    ) -> Result<()> {
        // Create H3 connection from quinn connection
        let mut conn = H3ServerConnection::new(h3_quinn::Connection::new(connection))
            .await
            .context("Failed to create H3 connection")?;

        let client = Arc::new(client.clone());

        loop {
            match conn.accept().await {
                Ok(Some(resolver)) => {
                    let rewriter = Arc::clone(&rewriter);
                    let client = Arc::clone(&client);
                    let metrics = Arc::clone(&metrics);
                    tokio::spawn(async move {
                        // Resolve the request
                        match resolver.resolve_request().await {
                            Ok((req, stream)) => {
                                if let Err(e) =
                                    Self::handle_request(req, stream, rewriter, &client, &metrics)
                                        .await
                                {
                                    error!("DoH3 request handling error: {}", e);
                                } else {
                                    debug!("DoH3 request handled successfully");
                                }
                            }
                            Err(e) => {
                                error!("DoH3 request resolution error: {}", e);
                            }
                        }
                    });
                }
                Ok(None) => {
                    // Connection closed
                    debug!("DoH3 connection closed by client");
                    break;
                }
                Err(e) => {
                    error!("DoH3 connection accept error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_request(
        req: hyper::Request<()>,
        mut stream: h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
        rewriter: SniRewriterType,
        client: &HttpClient,
        metrics: &Arc<Metrics>,
    ) -> Result<()> {
        let timer = Timer::start();
        let method = req.method().clone();
        let uri = req.uri().clone();
        info!("New DoH3 request: {} {}", method, uri);

        let host = req
            .headers()
            .get("host")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Missing or invalid Host header in {} request to {}",
                    method,
                    uri
                )
            })
            .context("Failed to extract Host header from DoH3 request")?;

        debug!("Processing DoH3 request for host: {}", host);

        let rewrite_result = rewriter
            .rewrite(host)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "SNI rewrite failed for hostname: {} (no matching base domain found)",
                    host
                )
            })
            .context("SNI rewrite operation failed for DoH3 request")?;

        // Record SNI rewrite
        metrics.record_sni_rewrite();

        info!(
            "DoH3 request: {} {} -> SNI rewrite: {} -> {} -> Target: {}",
            method,
            uri.path(),
            rewrite_result.original,
            rewrite_result.prefix,
            rewrite_result.target_hostname
        );

        // Build upstream URI without unnecessary allocation
        let path_and_query = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        let upstream_uri = format!(
            "https://{}{}",
            rewrite_result.target_hostname, path_and_query
        );

        debug!("Forwarding DoH3 request to upstream: {}", upstream_uri);

        // Read request body if POST (zerocopy where possible)
        let body = if *req.method() == Method::POST {
            let mut body_data = Vec::new();
            loop {
                match stream.recv_data().await {
                    Ok(Some(mut chunk)) => {
                        while chunk.has_remaining() {
                            body_data.extend_from_slice(chunk.chunk());
                            chunk.advance(chunk.chunk().len());
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        return Err(anyhow::anyhow!("Failed to read DoH3 request body: {}", e))
                            .context("Error reading request body from DoH3 stream");
                    }
                }
            }
            debug!("Read DoH3 request body: {} bytes", body_data.len());
            Bytes::from(body_data)
        } else {
            Bytes::new()
        };

        let bytes_received = body.len() as u64;

        // Forward request to upstream
        let result = forward_http_request(
            client,
            &upstream_uri,
            &rewrite_result.target_hostname,
            req.method().clone(),
            req.headers(),
            body,
        )
        .await;

        let duration = timer.elapsed();

        let response = match result {
            Ok((resp, bytes_sent)) => {
                metrics.record_request(true, bytes_received, bytes_sent, duration);
                resp
            }
            Err(e) => {
                debug!("DoH3 upstream request failed: {}", e);
                metrics.record_request(false, bytes_received, 0, duration);
                metrics.record_upstream_error();
                return Err(e).with_context(|| {
                    format!(
                        "Failed to forward DoH3 request to upstream: {}",
                        upstream_uri
                    )
                });
            }
        };

        debug!("Received response from upstream, sending to DoH3 client");

        // Send response back to client
        stream
            .send_response(response.map(|_| ()))
            .await
            .context("Failed to send DoH3 response to client")?;

        stream
            .finish()
            .await
            .context("Failed to finish DoH3 response stream")?;

        Ok(())
    }
}
