use dns_proxy::upstream::pool::ConnectionPool;
use std::sync::Arc;

#[test]
fn test_get_client_reuse() {
    let pool = ConnectionPool::new();
    let sni = "example.com";

    let client1 = pool.get_client(sni);
    let client2 = pool.get_client(sni);

    // Should return the same client instance
    assert_eq!(Arc::as_ptr(&client1), Arc::as_ptr(&client2));
}

#[test]
fn test_multiple_snis() {
    let pool = ConnectionPool::new();

    let client1 = pool.get_client("example.com");
    let client2 = pool.get_client("example.org");

    // Should be different clients
    assert_ne!(Arc::as_ptr(&client1), Arc::as_ptr(&client2));
}

#[test]
fn test_connection_pool_default() {
    let _pool = ConnectionPool::default();
}

#[test]
fn test_connection_pool_with_config() {
    use std::time::Duration;

    let pool = ConnectionPool::with_config(Duration::from_secs(30), Duration::from_secs(5), 5);

    // Test that it works
    let _client = pool.get_client("example.com");
}
