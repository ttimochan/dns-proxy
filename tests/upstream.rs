use dns_proxy::upstream::pool::{ConnectionPool, HttpClient};
use dns_proxy::upstream::{create_connection_pool, forward_http_request};

#[test]
fn test_create_connection_pool() {
    let _pool = create_connection_pool();
}

#[test]
fn test_upstream_module_imports() {
    // Test that upstream module exports are accessible
    // Verify the module structure exists
    let pool = create_connection_pool();
    let _client = pool.get_client("example.com");
    assert!(std::any::type_name::<HttpClient>().contains("Client"));
    assert!(std::any::type_name::<ConnectionPool>().contains("ConnectionPool"));
}

#[tokio::test]
async fn test_forward_http_request_invalid_uri() {
    use bytes::Bytes;
    use hyper::HeaderMap;
    use hyper::Method;

    let pool = create_connection_pool();
    let headers = HeaderMap::new();

    // Test with invalid URI - should handle gracefully
    let result = forward_http_request(
        &pool,
        "not-a-valid-uri",
        "example.com",
        Method::GET,
        &headers,
        Bytes::new(),
    )
    .await;

    // Should return an error or error response (BAD_GATEWAY)
    match result {
        Ok((resp, _)) => {
            // If it succeeds, it should be an error response
            assert!(resp.status().is_client_error() || resp.status().is_server_error());
        }
        Err(_) => {
            // Error is also acceptable for invalid URI
        }
    }
}
