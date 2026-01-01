use dns_proxy::config::{AppConfig, RewriteConfig};
use dns_proxy::metrics::Metrics;
use dns_proxy::readers::{DoH3Server, DoHServer, DoQServer, DoTServer, HealthcheckServer};
use dns_proxy::rewrite::create_rewriter;
use std::sync::Arc;

fn create_test_rewriter() -> dns_proxy::rewrite::SniRewriterType {
    create_rewriter(RewriteConfig {
        base_domains: vec!["example.com".to_string()],
        target_suffix: ".example.cn".to_string(),
        rewrite_failure_strategy: "error".to_string(),
    })
}

#[test]
fn test_healthcheck_server_new() {
    let config = Arc::new(AppConfig::default());
    let metrics = Arc::new(Metrics::new());
    let _server = HealthcheckServer::new(config, metrics);
    // Just verify it can be created without panicking
}

#[test]
fn test_dot_server_new() {
    let config = Arc::new(AppConfig::default());
    let rewriter = create_test_rewriter();
    let metrics = Arc::new(Metrics::new());
    let _server = DoTServer::new(config, rewriter, metrics);
    // Just verify it can be created without panicking
}

#[test]
fn test_doh_server_new() {
    let config = Arc::new(AppConfig::default());
    let rewriter = create_test_rewriter();
    let metrics = Arc::new(Metrics::new());
    let _server = DoHServer::new(config, rewriter, metrics);
    // Just verify it can be created without panicking
}

#[test]
fn test_doq_server_new() {
    let config = Arc::new(AppConfig::default());
    let rewriter = create_test_rewriter();
    let metrics = Arc::new(Metrics::new());
    let _server = DoQServer::new(config, rewriter, metrics);
    // Just verify it can be created without panicking
}

#[test]
fn test_doh3_server_new() {
    let config = Arc::new(AppConfig::default());
    let rewriter = create_test_rewriter();
    let metrics = Arc::new(Metrics::new());
    let _server = DoH3Server::new(config, rewriter, metrics);
    // Just verify it can be created without panicking
}

#[tokio::test]
async fn test_healthcheck_server_start_disabled() {
    let mut config = AppConfig::default();
    config.servers.healthcheck.enabled = false;
    let config = Arc::new(config);
    let metrics = Arc::new(Metrics::new());
    let server = HealthcheckServer::new(config, metrics);

    // Should return Ok immediately when disabled
    let result = server.start().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_dot_server_start_disabled() {
    let mut config = AppConfig::default();
    config.servers.dot.enabled = false;
    let config = Arc::new(config);
    let rewriter = create_test_rewriter();
    let metrics = Arc::new(Metrics::new());
    let server = DoTServer::new(config, rewriter, metrics);

    // Should return Ok immediately when disabled
    let result = server.start().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_doh_server_start_disabled() {
    let mut config = AppConfig::default();
    config.servers.doh.enabled = false;
    let config = Arc::new(config);
    let rewriter = create_test_rewriter();
    let metrics = Arc::new(Metrics::new());
    let server = DoHServer::new(config, rewriter, metrics);

    // Should return Ok immediately when disabled
    let result = server.start().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_doq_server_start_disabled() {
    let mut config = AppConfig::default();
    config.servers.doq.enabled = false;
    let config = Arc::new(config);
    let rewriter = create_test_rewriter();
    let metrics = Arc::new(Metrics::new());
    let server = DoQServer::new(config, rewriter, metrics);

    // Should return Ok immediately when disabled
    let result = server.start().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_doh3_server_start_disabled() {
    let mut config = AppConfig::default();
    config.servers.doh3.enabled = false;
    let config = Arc::new(config);
    let rewriter = create_test_rewriter();
    let metrics = Arc::new(Metrics::new());
    let server = DoH3Server::new(config, rewriter, metrics);

    // Should return Ok immediately when disabled
    let result = server.start().await;
    assert!(result.is_ok());
}
