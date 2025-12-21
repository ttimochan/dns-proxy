use crate::config::AppConfig;
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
use tracing::{error, info};

pub struct DoH3Server {
    config: Arc<AppConfig>,
    rewriter: SniRewriterType,
    client: HttpClient,
}

impl DoH3Server {
    pub fn new(config: Arc<AppConfig>, rewriter: SniRewriterType) -> Self {
        Self {
            config,
            rewriter,
            client: create_http_client(),
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

        loop {
            match endpoint.accept().await {
                Some(conn) => {
                    let rewriter = Arc::clone(&rewriter);
                    let client = Arc::clone(&client);
                    tokio::spawn(async move {
                        match conn.await {
                            Ok(connection) => {
                                info!("New DoH3 connection from {}", connection.remote_address());
                                if let Err(e) =
                                    Self::handle_connection(connection, rewriter, &*client).await
                                {
                                    error!("DoH3 connection error: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("DoH3 connection error: {}", e);
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
        rewriter: SniRewriterType,
        client: &HttpClient,
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
                    tokio::spawn(async move {
                        // Resolve the request
                        match resolver.resolve_request().await {
                            Ok((req, stream)) => {
                                if let Err(e) =
                                    Self::handle_request(req, stream, rewriter, &*client).await
                                {
                                    error!("DoH3 request error: {}", e);
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
                    break;
                }
                Err(e) => {
                    error!("DoH3 accept error: {}", e);
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
    ) -> Result<()> {
        info!("New DoH3 request: {} {}", req.method(), req.uri());

        let host = req
            .headers()
            .get("host")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid Host header"))?;

        let rewrite_result = rewriter
            .rewrite(host)
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to rewrite hostname: {}", host))?;

        info!(
            "DoH3: {} -> Prefix: {} -> Target: {}",
            rewrite_result.original, rewrite_result.prefix, rewrite_result.target_hostname
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
                        return Err(anyhow::anyhow!("Failed to read request body: {}", e));
                    }
                }
            }
            Bytes::from(body_data)
        } else {
            Bytes::new()
        };

        // Forward request to upstream
        let response = forward_http_request(
            client,
            &upstream_uri,
            &rewrite_result.target_hostname,
            req.method().clone(),
            req.headers(),
            body,
        )
        .await
        .map_err(|e| anyhow::anyhow!("HTTP upstream error: {}", e))?;

        // Send response back to client
        stream
            .send_response(response.map(|_| ()))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send response: {}", e))?;

        stream
            .finish()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to finish stream: {}", e))?;

        Ok(())
    }
}
