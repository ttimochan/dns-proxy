use crate::metrics::{Metrics, Timer};
use crate::rewrite::SniRewriterType;
use crate::sni::SniRewriter;
use crate::upstream::http::{HttpClient, forward_http_request};
use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::{Method, Request, Response};
use std::sync::Arc;
use tracing::{debug, info};

/// Handle HTTP request with SNI rewriting and upstream forwarding
pub async fn handle_http_request(
    req: Request<Incoming>,
    rewriter: SniRewriterType,
    client: &HttpClient,
    metrics: Arc<Metrics>,
) -> Result<Response<http_body_util::Full<hyper::body::Bytes>>> {
    let timer = Timer::start();
    let method = req.method().clone();
    let uri = req.uri().clone();

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
        .context("Failed to extract Host header from request")?;

    debug!("Processing {} request for host: {}", method, host);

    let rewrite_result = rewriter
        .rewrite(host)
        .await
        .ok_or_else(|| {
            anyhow::anyhow!(
                "SNI rewrite failed for hostname: {} (no matching base domain found)",
                host
            )
        })
        .context("SNI rewrite operation failed")?;

    // Record SNI rewrite
    metrics.record_sni_rewrite();

    info!(
        "HTTP request: {} {} -> SNI rewrite: {} -> {} -> Target: {}",
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

    debug!("Forwarding request to upstream: {}", upstream_uri);

    // Extract headers before consuming request
    let headers = req.headers().clone();

    // Extract body if POST (zerocopy: reuse bytes when possible)
    let body = if method == Method::POST {
        req.into_body()
            .collect()
            .await
            .context("Failed to read request body")?
            .to_bytes()
    } else {
        Bytes::new()
    };

    debug!("Request body size: {} bytes", body.len());

    let bytes_received = body.len() as u64;

    // Forward request
    let result = forward_http_request(
        client,
        &upstream_uri,
        &rewrite_result.target_hostname,
        method,
        &headers,
        body,
    )
    .await;

    let duration = timer.elapsed();

    // Record metrics and extract response
    match result {
        Ok((response, bytes_sent)) => {
            metrics.record_request(true, bytes_received, bytes_sent, duration);
            Ok(response)
        }
        Err(e) => {
            debug!("HTTP request failed: {}", e);
            metrics.record_request(false, bytes_received, 0, duration);
            metrics.record_upstream_error();
            Err(e).with_context(|| {
                format!(
                    "Failed to forward HTTP request to upstream: {}",
                    upstream_uri
                )
            })
        }
    }
}
