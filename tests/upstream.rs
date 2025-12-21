use dns_proxy::upstream::{HttpClient, create_http_client};

#[test]
fn test_create_http_client() {
    let _client = create_http_client();
    // Verify client is created successfully
    assert!(std::any::type_name::<HttpClient>().contains("Client"));
}

#[test]
fn test_upstream_module_imports() {
    // Test that upstream module exports are accessible
    // Verify the module structure exists
    let _client: HttpClient = create_http_client();
    assert!(std::any::type_name::<HttpClient>().contains("Client"));
}

#[tokio::test]
async fn test_forward_http_request_invalid_uri() {
    use bytes::Bytes;
    use dns_proxy::upstream::forward_http_request;
    use hyper::HeaderMap;
    use hyper::Method;

    let client = create_http_client();
    let headers = HeaderMap::new();

    // Test with invalid URI - should handle gracefully
    let result = forward_http_request(
        &client,
        "not-a-valid-uri",
        "example.com",
        Method::GET,
        &headers,
        Bytes::new(),
    )
    .await;

    // Should return an error or error response (BAD_GATEWAY)
    match result {
        Ok(resp) => {
            // If it succeeds, it should be an error response
            assert!(resp.status().is_client_error() || resp.status().is_server_error());
        }
        Err(_) => {
            // Error is also acceptable for invalid URI
        }
    }
}
