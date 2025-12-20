use crate::config::AppConfig;
use crate::rewrite::SniRewriterType;
use crate::sni::SniRewriter;
use anyhow::{Context, Result};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

pub struct DoHServer {
    config: Arc<AppConfig>,
    rewriter: SniRewriterType,
}

impl DoHServer {
    pub fn new(config: Arc<AppConfig>, rewriter: SniRewriterType) -> Self {
        Self { config, rewriter }
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

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let rewriter = Arc::clone(&self.rewriter);
                    let upstream_url = self.config.doh_upstream().to_string();
                    let config = Arc::clone(&self.config);
                    tokio::spawn(async move {
                        let io = TokioIo::new(stream);
                        let service = service_fn(move |req| {
                            let rewriter = Arc::clone(&rewriter);
                            let upstream_url = upstream_url.clone();
                            let config = Arc::clone(&config);
                            let addr = addr;
                            async move {
                                Self::handle_request(req, rewriter, &upstream_url, &config)
                                    .await
                                    .map_err(|e| {
                                        error!("DoH handler error from {}: {}", addr, e);
                                        std::io::Error::new(
                                            std::io::ErrorKind::Other,
                                            e.to_string(),
                                        )
                                    })
                            }
                        });

                        if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                            error!("DoH connection error from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("DoH accept error: {}", e);
                }
            }
        }
    }

    async fn handle_request(
        req: Request<Incoming>,
        rewriter: SniRewriterType,
        _upstream_url: &str,
        _config: &Arc<AppConfig>,
    ) -> Result<Response<Full<hyper::body::Bytes>>> {
        info!("New DoH request: {} {}", req.method(), req.uri());

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
            "DoH: {} -> Prefix: {} -> Target: {}",
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

        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http();

        match *req.method() {
            Method::GET => {
                Self::handle_get_request(
                    req,
                    &upstream_uri,
                    &rewrite_result.target_hostname,
                    &client,
                )
                .await
            }
            Method::POST => {
                Self::handle_post_request(
                    req,
                    &upstream_uri,
                    &rewrite_result.target_hostname,
                    &client,
                )
                .await
            }
            _ => Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Full::new("Method not allowed".into()))
                .unwrap()),
        }
    }

    async fn handle_get_request(
        req: Request<Incoming>,
        upstream_uri: &str,
        target_hostname: &str,
        client: &hyper_util::client::legacy::Client<
            hyper_util::client::legacy::connect::HttpConnector,
            Full<hyper::body::Bytes>,
        >,
    ) -> Result<Response<Full<hyper::body::Bytes>>> {
        let mut upstream_req = Request::builder()
            .method(Method::GET)
            .uri(upstream_uri)
            .body(Full::new(hyper::body::Bytes::new()))?;

        // Copy headers efficiently
        for (key, value) in req.headers() {
            upstream_req.headers_mut().insert(key, value.clone());
        }
        upstream_req
            .headers_mut()
            .insert("host", target_hostname.parse()?);

        Self::forward_request(client, upstream_req).await
    }

    async fn handle_post_request(
        req: Request<Incoming>,
        upstream_uri: &str,
        target_hostname: &str,
        client: &hyper_util::client::legacy::Client<
            hyper_util::client::legacy::connect::HttpConnector,
            Full<hyper::body::Bytes>,
        >,
    ) -> Result<Response<Full<hyper::body::Bytes>>> {
        let (parts, body) = req.into_parts();
        let body_bytes = body.collect().await?.to_bytes();

        let mut upstream_req = Request::builder()
            .method(Method::POST)
            .uri(upstream_uri)
            .body(Full::new(body_bytes))?;

        // Copy headers efficiently
        for (key, value) in parts.headers.iter() {
            upstream_req.headers_mut().insert(key, value.clone());
        }
        upstream_req
            .headers_mut()
            .insert("host", target_hostname.parse()?);

        Self::forward_request(client, upstream_req).await
    }

    async fn forward_request(
        client: &hyper_util::client::legacy::Client<
            hyper_util::client::legacy::connect::HttpConnector,
            Full<hyper::body::Bytes>,
        >,
        upstream_req: Request<Full<hyper::body::Bytes>>,
    ) -> Result<Response<Full<hyper::body::Bytes>>> {
        match client.request(upstream_req).await {
            Ok(resp) => {
                let (parts, body) = resp.into_parts();
                let body_bytes = body.collect().await?.to_bytes();
                Ok(Response::from_parts(parts, Full::new(body_bytes)))
            }
            Err(e) => {
                error!("DoH upstream error: {}", e);
                Ok(Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Full::new(format!("Upstream error: {}", e).into()))
                    .unwrap())
            }
        }
    }
}
