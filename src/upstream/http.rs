use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use std::time::Duration;
use tracing::{debug, error, warn};

/// Default timeout for upstream requests (30 seconds)
const DEFAULT_UPSTREAM_TIMEOUT: Duration = Duration::from_secs(30);

/// Shared HTTP client for upstream requests
pub type HttpClient = Client<HttpConnector, Full<Bytes>>;

/// Create a new HTTP client instance
pub fn create_http_client() -> HttpClient {
    Client::builder(TokioExecutor::new()).build_http()
}

/// Forward HTTP request to upstream server with timeout control
/// Returns the response and the body size in bytes for metrics
pub async fn forward_http_request(
    client: &HttpClient,
    upstream_uri: &str,
    target_hostname: &str,
    method: Method,
    headers: &hyper::HeaderMap,
    body: Bytes,
) -> Result<(Response<Full<Bytes>>, u64)> {
    let mut req = Request::builder()
        .method(method.clone())
        .uri(upstream_uri)
        .body(Full::new(body.clone()))
        .with_context(|| {
            format!(
                "Failed to build HTTP request: {} {} (target: {})",
                method, upstream_uri, target_hostname
            )
        })?;

    // Copy headers efficiently - only copy necessary headers
    // Skip headers that will be overwritten or aren't needed
    let skip_headers = ["host", "connection", "keep-alive", "transfer-encoding"];
    for (key, value) in headers {
        let key_str = key.as_str();
        if !skip_headers.contains(&key_str) {
            // Use reference to avoid cloning when possible
            req.headers_mut().insert(key, value.clone());
        }
    }

    req.headers_mut().insert(
        "host",
        target_hostname
            .parse()
            .with_context(|| format!("Invalid target hostname: {}", target_hostname))?,
    );

    debug!(
        "Sending {} request to upstream: {} (Host: {})",
        method, upstream_uri, target_hostname
    );

    // Add timeout control to prevent hanging requests
    let request_future = client.request(req);
    let timeout_future = tokio::time::timeout(DEFAULT_UPSTREAM_TIMEOUT, request_future);

    match timeout_future.await {
        Ok(Ok(resp)) => {
            let status = resp.status();
            let (parts, body) = resp.into_parts();

            debug!(
                "Received response from upstream: {} {}",
                status, upstream_uri
            );

            let body_bytes = body
                .collect()
                .await
                .with_context(|| {
                    format!(
                        "Failed to read response body from upstream: {}",
                        upstream_uri
                    )
                })?
                .to_bytes();

            let body_size = body_bytes.len() as u64;
            debug!("Response body size: {} bytes", body_size);

            if !status.is_success() {
                warn!(
                    "Upstream returned non-success status: {} {} (body: {} bytes)",
                    status, upstream_uri, body_size
                );
            }

            Ok((
                Response::from_parts(parts, Full::new(body_bytes)),
                body_size,
            ))
        }
        Ok(Err(e)) => {
            error!(
                "HTTP upstream request failed: {} {} -> {} (target: {})",
                method, upstream_uri, e, target_hostname
            );

            // Return a proper error response instead of panicking
            let error_msg = format!("Upstream error: {}", e);
            let error_body = Full::new(error_msg.clone().into());
            let error_size = error_msg.len() as u64;
            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(error_body)
                .map(|resp| (resp, error_size))
                .with_context(|| {
                    format!(
                        "Failed to create error response for upstream failure: {}",
                        upstream_uri
                    )
                })
        }
        Err(_) => {
            error!(
                "HTTP upstream request timeout: {} {} (target: {}, timeout: {:?})",
                method, upstream_uri, target_hostname, DEFAULT_UPSTREAM_TIMEOUT
            );

            // Return timeout error response
            let error_msg = format!("Upstream timeout after {:?}", DEFAULT_UPSTREAM_TIMEOUT);
            let error_body = Full::new(error_msg.clone().into());
            let error_size = error_msg.len() as u64;
            Response::builder()
                .status(StatusCode::GATEWAY_TIMEOUT)
                .body(error_body)
                .map(|resp| (resp, error_size))
                .with_context(|| {
                    format!(
                        "Failed to create timeout response for upstream: {}",
                        upstream_uri
                    )
                })
        }
    }
}
