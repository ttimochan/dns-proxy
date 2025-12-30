use crate::config::AppConfig;
use crate::metrics::Metrics;
use anyhow::{Context, Result};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

pub struct HealthcheckServer {
    config: Arc<AppConfig>,
    metrics: Arc<Metrics>,
}

impl HealthcheckServer {
    pub fn new(config: Arc<AppConfig>, metrics: Arc<Metrics>) -> Self {
        Self { config, metrics }
    }

    pub async fn start(&self) -> Result<()> {
        let server_config = &self.config.servers.healthcheck;
        if !server_config.enabled {
            info!("Healthcheck server is disabled");
            return Ok(());
        }

        let bind_addr = format!("{}:{}", server_config.bind_address, server_config.port);
        let listener = TcpListener::bind(&bind_addr)
            .await
            .with_context(|| format!("Failed to bind healthcheck server to {}", bind_addr))?;

        info!(
            "Healthcheck server listening on {}:{} at path {}",
            server_config.bind_address, server_config.port, server_config.path
        );

        let healthcheck_path = server_config.path.clone();
        let metrics = Arc::clone(&self.metrics);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let path = healthcheck_path.clone();
                    let client_addr = addr;
                    let metrics = Arc::clone(&metrics);
                    tokio::spawn(async move {
                        let io = TokioIo::new(stream);
                        let service = service_fn(move |req| {
                            let path = path.clone();
                            let addr = client_addr;
                            let metrics = Arc::clone(&metrics);
                            async move {
                                handle_healthcheck(req, &path, &metrics).await.map_err(|e| {
                                    error!("Healthcheck handler error from {}: {}", addr, e);
                                    std::io::Error::other(e.to_string())
                                })
                            }
                        });

                        if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                            error!("Healthcheck connection error from {}: {}", client_addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Healthcheck accept error: {}", e);
                }
            }
        }
    }
}

async fn handle_healthcheck(
    req: Request<hyper::body::Incoming>,
    healthcheck_path: &str,
    metrics: &Metrics,
) -> Result<Response<Full<Bytes>>, std::io::Error> {
    // Only handle GET requests
    if req.method() != Method::GET {
        return Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Full::new(Bytes::from("Method not allowed")))
            .unwrap());
    }

    // Check if the path matches the healthcheck path or metrics path
    let path = req.uri().path();

    // Handle metrics endpoint
    if path == "/metrics" || path == "/stats" {
        let snapshot = metrics.snapshot();
        let response = serde_json::json!({
            "total_requests": snapshot.total_requests,
            "successful_requests": snapshot.successful_requests,
            "failed_requests": snapshot.failed_requests,
            "bytes_received": snapshot.bytes_received,
            "bytes_sent": snapshot.bytes_sent,
            "sni_rewrites": snapshot.sni_rewrites,
            "upstream_errors": snapshot.upstream_errors,
            "average_processing_time_ms": snapshot.average_processing_time_ms,
            "success_rate": snapshot.success_rate
        });

        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(response.to_string())))
            .unwrap());
    }

    if path != healthcheck_path {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("Not found")))
            .unwrap());
    }

    // Return healthy status
    let response = serde_json::json!({
        "status": "healthy",
        "service": "dns-proxy"
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(response.to_string())))
        .unwrap())
}
