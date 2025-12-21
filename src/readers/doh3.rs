use crate::config::AppConfig;
use crate::rewrite::SniRewriterType;
use crate::sni::SniRewriter;
use crate::tls_utils;
use anyhow::{Context, Result};
use bytes::{Buf, Bytes};
use h3::server::Connection as H3ServerConnection;
use http_body_util::Full;
use hyper::{Method, Response, StatusCode};
use quinn::crypto::rustls::QuicServerConfig;
use quinn::{Endpoint, ServerConfig};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info};

pub struct DoH3Server {
    config: Arc<AppConfig>,
    rewriter: SniRewriterType,
}

impl DoH3Server {
    pub fn new(config: Arc<AppConfig>, rewriter: SniRewriterType) -> Self {
        Self { config, rewriter }
    }

    pub async fn start(&self) -> Result<()> {
        let server_config = &self.config.servers.doh3;
        if !server_config.enabled {
            info!("DoH3 server is disabled");
            return Ok(());
        }

        // Create QUIC server configuration
        let rustls_config = tls_utils::create_server_config(self.config.as_ref())
            .await
            .context("Failed to create TLS server config for DoH3")?;

        // Convert rustls::ServerConfig to quinn::rustls::ServerConfig
        let rustls_config_arc = Arc::new(rustls_config);
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
            .context("Failed to create QUIC endpoint for DoH3")?;

        info!("DoH3 server listening on UDP {}", addr);

        let rewriter = Arc::clone(&self.rewriter);
        let config = Arc::clone(&self.config);

        loop {
            match endpoint.accept().await {
                Some(conn) => {
                    let rewriter = Arc::clone(&rewriter);
                    let config = Arc::clone(&config);
                    tokio::spawn(async move {
                        match conn.await {
                            Ok(connection) => {
                                info!("New DoH3 connection from {}", connection.remote_address());
                                if let Err(e) =
                                    Self::handle_connection(connection, rewriter, config).await
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
        config: Arc<AppConfig>,
    ) -> Result<()> {
        // Create H3 connection from quinn connection
        let mut conn = H3ServerConnection::new(h3_quinn::Connection::new(connection))
            .await
            .context("Failed to create H3 connection")?;

        loop {
            match conn.accept().await {
                Ok(Some(resolver)) => {
                    let rewriter = Arc::clone(&rewriter);
                    let config = Arc::clone(&config);
                    tokio::spawn(async move {
                        // Resolve the request
                        match resolver.resolve_request().await {
                            Ok((req, stream)) => {
                                if let Err(e) =
                                    Self::handle_request(req, stream, rewriter, config).await
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
        _config: Arc<AppConfig>,
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

        let path_and_query = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        let upstream_uri = format!(
            "https://{}{}",
            rewrite_result.target_hostname, path_and_query
        );

        // Read request body if POST
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

        // Forward request to upstream DoH3 server
        let response = match *req.method() {
            Method::GET => {
                Self::handle_get_request(&upstream_uri, &rewrite_result.target_hostname).await?
            }
            Method::POST => {
                Self::handle_post_request(&upstream_uri, &rewrite_result.target_hostname, body)
                    .await?
            }
            _ => Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(())
                .unwrap(),
        };

        // Send response back to client
        stream
            .send_response(response)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send response: {}", e))?;

        // Note: Response body is sent separately using send_data if needed
        // For now, we'll just finish the stream
        stream
            .finish()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to finish stream: {}", e))?;

        Ok(())
    }

    async fn handle_get_request(upstream_uri: &str, target_hostname: &str) -> Result<Response<()>> {
        // Create HTTP/3 client and forward request
        // For now, we'll use a simple HTTP client approach
        // In a full implementation, you'd use h3 client here
        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http();

        let mut upstream_req = hyper::Request::builder()
            .method(Method::GET)
            .uri(upstream_uri)
            .body(Full::new(hyper::body::Bytes::new()))?;

        upstream_req
            .headers_mut()
            .insert("host", target_hostname.parse()?);

        match client.request(upstream_req).await {
            Ok(resp) => {
                let (parts, _body) = resp.into_parts();
                // Note: We're not sending the body in this simplified version
                // In a full implementation, you'd send the body using stream.send_data()
                Ok(Response::from_parts(parts, ()))
            }
            Err(e) => {
                error!("DoH3 upstream error: {}", e);
                Ok(Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(())
                    .unwrap())
            }
        }
    }

    async fn handle_post_request(
        upstream_uri: &str,
        target_hostname: &str,
        body: Bytes,
    ) -> Result<Response<()>> {
        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http();

        let mut upstream_req = hyper::Request::builder()
            .method(Method::POST)
            .uri(upstream_uri)
            .body(Full::new(body))?;

        upstream_req
            .headers_mut()
            .insert("host", target_hostname.parse()?);

        match client.request(upstream_req).await {
            Ok(resp) => {
                let (parts, _body) = resp.into_parts();
                // Note: We're not sending the body in this simplified version
                // In a full implementation, you'd send the body using stream.send_data()
                Ok(Response::from_parts(parts, ()))
            }
            Err(e) => {
                error!("DoH3 upstream error: {}", e);
                Ok(Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(())
                    .unwrap())
            }
        }
    }
}
