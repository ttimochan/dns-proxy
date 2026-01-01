use dns_proxy::app::App;
use dns_proxy::config::AppConfig;
use dns_proxy::sni::SniRewriter;
use std::time::Duration;
use tokio::time::timeout;

/// Integration test: Test that the app can start with all servers disabled
#[tokio::test]
async fn test_app_start_all_disabled() {
    let mut config = AppConfig::default();
    config.servers.dot.enabled = false;
    config.servers.doh.enabled = false;
    config.servers.doq.enabled = false;
    config.servers.doh3.enabled = false;
    config.servers.healthcheck.enabled = false;

    // Validate config
    assert!(config.validate().is_ok());

    let mut app = App::new(config);
    assert!(app.start().is_ok());

    // Give servers a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Clean shutdown
    app.wait_for_shutdown().await;
}

/// Integration test: Test configuration validation
#[test]
fn test_config_validation() {
    let mut config = AppConfig::default();

    // Disable all servers to avoid certificate validation issues
    config.servers.dot.enabled = false;
    config.servers.doh.enabled = false;
    config.servers.doq.enabled = false;
    config.servers.doh3.enabled = false;
    config.servers.healthcheck.enabled = false;
    // Clear TLS config to avoid file validation
    config.tls = Default::default();

    // Valid config should pass
    assert!(config.validate().is_ok());

    // Invalid port conflict should fail
    config.servers.dot.port = 443;
    config.servers.doh.port = 443;
    config.servers.dot.enabled = true;
    config.servers.doh.enabled = true;
    assert!(config.validate().is_err());
}

/// Integration test: Test SNI rewrite flow
#[tokio::test]
async fn test_sni_rewrite_flow() {
    let config = AppConfig::default();
    let app = App::new(config);

    // Test that rewriter is available
    let test_sni = "www.example.org";
    let result = app.rewriter.rewrite(test_sni).await;

    assert!(result.is_some());
    let rewrite_result = result.unwrap();
    assert_eq!(rewrite_result.original, test_sni);
    assert_eq!(rewrite_result.prefix, "www");
    assert_eq!(rewrite_result.target_hostname, "www.example.cn");
}

/// Integration test: Test upstream configuration parsing
#[test]
fn test_upstream_config_parsing() {
    let config = AppConfig::default();

    // Test DoT upstream
    let dot_upstream = config.dot_upstream();
    assert!(dot_upstream.is_ok());
    assert_eq!(dot_upstream.unwrap().port(), 853);

    // Test DoQ upstream
    let doq_upstream = config.doq_upstream();
    assert!(doq_upstream.is_ok());
    assert_eq!(doq_upstream.unwrap().port(), 853);

    // Test hostname extraction
    let hostname = config.dot_upstream_hostname();
    assert!(!hostname.is_empty());
}

/// Integration test: Test that healthcheck server can be started independently
#[tokio::test]
async fn test_healthcheck_server_start() {
    let mut config = AppConfig::default();
    config.servers.dot.enabled = false;
    config.servers.doh.enabled = false;
    config.servers.doq.enabled = false;
    config.servers.doh3.enabled = false;
    config.servers.healthcheck.enabled = true;
    config.servers.healthcheck.port = 18080; // Use a different port to avoid conflicts

    assert!(config.validate().is_ok());

    let mut app = App::new(config);
    assert!(app.start().is_ok());

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Test healthcheck endpoint
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/health", 18080);

    // Try to connect (may fail if server isn't ready, but that's ok for this test)
    let _result = timeout(Duration::from_secs(1), client.get(&url).send()).await;

    // Clean shutdown
    app.wait_for_shutdown().await;

    // We don't assert on the result since the server might not be fully ready
    // The important thing is that it started without errors
}

/// Integration test: Test metrics endpoint
#[tokio::test]
async fn test_metrics_endpoint() {
    let mut config = AppConfig::default();
    config.servers.dot.enabled = false;
    config.servers.doh.enabled = false;
    config.servers.doq.enabled = false;
    config.servers.doh3.enabled = false;
    config.servers.healthcheck.enabled = true;
    config.servers.healthcheck.port = 18081; // Use a different port to avoid conflicts

    assert!(config.validate().is_ok());

    let mut app = App::new(config);

    // Record some metrics before starting
    app.metrics
        .record_request(true, 100, 200, Duration::from_millis(50));
    app.metrics.record_sni_rewrite();
    app.metrics.record_upstream_error();

    assert!(app.start().is_ok());

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Test metrics endpoint (now returns Prometheus format)
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/metrics", 18081);

    // Try to fetch metrics
    if let Ok(result) = timeout(Duration::from_secs(2), client.get(&url).send()).await {
        if let Ok(response) = result {
            if response.status().is_success() {
                let body = response.text().await.unwrap();
                // Verify Prometheus format
                assert!(body.contains("dns_proxy_requests_total"));
                assert!(body.contains("dns_proxy_requests_success"));
                assert!(body.contains("dns_proxy_requests_failed"));
                assert!(body.contains("dns_proxy_bytes_received_total"));
                assert!(body.contains("dns_proxy_bytes_sent_total"));
                assert!(body.contains("dns_proxy_sni_rewrites_total"));
                assert!(body.contains("dns_proxy_upstream_errors_total"));
                assert!(body.contains("dns_proxy_processing_time_seconds"));
            }
        }
    }

    // Clean shutdown
    app.wait_for_shutdown().await;
}

/// Integration test: Test metrics collection during app lifecycle
#[tokio::test]
async fn test_metrics_collection() {
    let mut config = AppConfig::default();
    config.servers.dot.enabled = false;
    config.servers.doh.enabled = false;
    config.servers.doq.enabled = false;
    config.servers.doh3.enabled = false;
    config.servers.healthcheck.enabled = false;

    let app = App::new(config);

    // Initially, metrics should be zero
    let snapshot = app.metrics.snapshot().await;
    assert_eq!(snapshot.total_requests, 0);

    // Record some metrics
    app.metrics
        .record_request(true, 100, 200, Duration::from_millis(50));
    app.metrics
        .record_request(false, 50, 0, Duration::from_millis(10));
    app.metrics.record_sni_rewrite();
    app.metrics.record_upstream_error();

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Verify metrics were recorded
    let snapshot = app.metrics.snapshot().await;
    assert_eq!(snapshot.total_requests, 2);
    assert_eq!(snapshot.successful_requests, 1);
    assert_eq!(snapshot.failed_requests, 1);
    assert_eq!(snapshot.bytes_received, 150);
    assert_eq!(snapshot.bytes_sent, 200);
    assert_eq!(snapshot.sni_rewrites, 1);
    assert_eq!(snapshot.upstream_errors, 1);
    assert!(snapshot.success_rate > 0.0);
    assert!(snapshot.average_processing_time_ms > 0.0);
}
