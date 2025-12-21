use crate::rewrite::SniRewriterType;
use crate::sni::SniRewriter;
use crate::upstream::http::{HttpClient, forward_http_request};
use anyhow::Result;
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::{Method, Request, Response};
use tracing::info;

/// Handle HTTP request with SNI rewriting and upstream forwarding
pub async fn handle_http_request(
    req: Request<Incoming>,
    rewriter: SniRewriterType,
    client: &HttpClient,
) -> Result<Response<http_body_util::Full<hyper::body::Bytes>>> {
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
        "HTTP: {} -> Prefix: {} -> Target: {}",
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

    // Extract method and headers before consuming request
    let method = req.method().clone();
    let headers = req.headers().clone();

    // Extract body if POST (zerocopy: reuse bytes when possible)
    let body = if method == Method::POST {
        req.into_body().collect().await?.to_bytes()
    } else {
        Bytes::new()
    };

    // Forward request
    forward_http_request(
        client,
        &upstream_uri,
        &rewrite_result.target_hostname,
        method,
        &headers,
        body,
    )
    .await
    .map_err(|e| anyhow::anyhow!("HTTP upstream error: {}", e))
}
