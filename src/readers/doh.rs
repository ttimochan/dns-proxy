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
use tokio::net::TcpListener;
use tracing::{error, info};

pub struct DoHServer {
    config: AppConfig,
    rewriter: SniRewriterType,
}

impl DoHServer {
    pub fn new(config: AppConfig, rewriter: SniRewriterType) -> Self {
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
                Ok((stream, _addr)) => {
                    let rewriter = self.rewriter.clone();
                    let upstream_url = self.config.doh_upstream().to_string();
                    tokio::spawn(async move {
                        let io = TokioIo::new(stream);
                        let service = service_fn(move |req| {
                            let rewriter = rewriter.clone();
                            let upstream_url = upstream_url.clone();
                            async move {
                                Self::handle_request(req, rewriter, &upstream_url)
                                    .await
                                    .map_err(|e| {
                                        error!("DoH handler error: {}", e);
                                        std::io::Error::new(
                                            std::io::ErrorKind::Other,
                                            e.to_string(),
                                        )
                                    })
                            }
                        });

                        if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                            error!("DoH connection error: {}", e);
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
    ) -> Result<Response<Full<hyper::body::Bytes>>> {
        info!("New DoH request: {} {}", req.method(), req.uri());

        let host = req
            .headers()
            .get("host")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        let rewrite_result = if let Some(result) = rewriter.rewrite(host).await {
            result
        } else {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new("Invalid hostname".into()))
                .unwrap());
        };

        info!(
            "DoH: {} -> Prefix: {} -> Target: {}",
            rewrite_result.original, rewrite_result.prefix, rewrite_result.target_hostname
        );

        let uri = req.uri().clone();
        let upstream_uri = format!(
            "https://{}{}",
            rewrite_result.target_hostname,
            uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("")
        );

        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http();

        if req.method() == Method::GET {
            let mut upstream_req = Request::builder()
                .method(Method::GET)
                .uri(&upstream_uri)
                .body(Full::new(hyper::body::Bytes::new()))?;

            *upstream_req.headers_mut() = req.headers().clone();
            upstream_req
                .headers_mut()
                .insert("host", rewrite_result.target_hostname.parse()?);

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
        } else if req.method() == Method::POST {
            let headers = req.headers().clone();
            let body = req.into_body().collect().await?.to_bytes();

            let mut upstream_req = Request::builder()
                .method(Method::POST)
                .uri(&upstream_uri)
                .body(Full::new(body))?;

            *upstream_req.headers_mut() = headers;
            upstream_req
                .headers_mut()
                .insert("host", rewrite_result.target_hostname.parse()?);

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
        } else {
            Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Full::new("Method not allowed".into()))
                .unwrap())
        }
    }
}
