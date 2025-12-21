use anyhow::Result;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use tracing::error;

/// Shared HTTP client for upstream requests
pub type HttpClient = Client<HttpConnector, Full<Bytes>>;

/// Create a new HTTP client instance
pub fn create_http_client() -> HttpClient {
    Client::builder(TokioExecutor::new()).build_http()
}

/// Forward HTTP request to upstream server
pub async fn forward_http_request(
    client: &HttpClient,
    upstream_uri: &str,
    target_hostname: &str,
    method: Method,
    headers: &hyper::HeaderMap,
    body: Bytes,
) -> Result<Response<Full<Bytes>>> {
    let mut req = Request::builder()
        .method(method)
        .uri(upstream_uri)
        .body(Full::new(body))
        .map_err(|e| anyhow::anyhow!("Failed to build request: {}", e))?;

    //TODO: Copy headers efficiently (zerocopy where possible)
    for (key, value) in headers {
        req.headers_mut().insert(key, value.clone());
    }
    req.headers_mut()
        .insert("host", target_hostname.parse().unwrap());

    match client.request(req).await {
        Ok(resp) => {
            let (parts, body) = resp.into_parts();
            let body_bytes = body.collect().await?.to_bytes();
            Ok(Response::from_parts(parts, Full::new(body_bytes)))
        }
        Err(e) => {
            error!("HTTP upstream error: {}", e);
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(format!("Upstream error: {}", e).into()))
                .unwrap())
        }
    }
}
