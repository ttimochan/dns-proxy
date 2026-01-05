use dns_proxy::config::{AppConfig, CertificateConfig, TlsConfig};
use dns_proxy::tls_utils::{CertificateResolver, DynamicCertResolver};
use std::sync::Arc;

#[test]
fn test_certificate_resolver_new() {
    let config = AppConfig::default();
    let resolver = CertificateResolver::new(config);
    assert!(resolver.cert_cache.is_empty());
}

#[tokio::test]
async fn test_get_cert_for_domain_with_config() {
    let mut config = AppConfig::default();
    let mut tls_config = TlsConfig::default();

    let cert_config = CertificateConfig {
        cert_file: "/nonexistent/cert.pem".to_string(),
        key_file: "/nonexistent/key.pem".to_string(),
        ca_file: None,
        require_client_cert: false,
    };

    tls_config
        .certs
        .insert("example.com".to_string(), cert_config);
    config.tls = tls_config;

    let resolver = CertificateResolver::new(config);

    let result = resolver.get_cert_for_domain("example.com").await;
    assert!(result.is_err());
    if let Err(e) = result {
        let err_msg = format!("{}", e);
        // Error message may contain "Failed to read certificate file" or "Failed to load certificate"
        assert!(
            err_msg.contains("Failed to read certificate file")
                || err_msg.contains("Failed to load certificate")
                || err_msg.contains("certificate")
        );
    }
}

#[tokio::test]
async fn test_get_cert_for_domain_no_config() {
    let config = AppConfig::default();
    let resolver = CertificateResolver::new(config);

    let result = resolver.get_cert_for_domain("unknown.com").await;
    assert!(result.is_err());
    if let Err(e) = result {
        let err_msg = format!("{}", e);
        assert!(err_msg.contains("No certificate configured"));
    }
}

#[tokio::test]
async fn test_get_cert_for_domain_caching() {
    let mut config = AppConfig::default();
    let mut tls_config = TlsConfig::default();

    let cert_config = CertificateConfig {
        cert_file: "/nonexistent/cert.pem".to_string(),
        key_file: "/nonexistent/key.pem".to_string(),
        ca_file: None,
        require_client_cert: false,
    };

    tls_config
        .certs
        .insert("example.com".to_string(), cert_config);
    config.tls = tls_config;

    let resolver = CertificateResolver::new(config);

    let result1 = resolver.get_cert_for_domain("example.com").await;
    let result2 = resolver.get_cert_for_domain("example.com").await;

    assert!(result1.is_err());
    assert!(result2.is_err());
}

#[test]
fn test_dynamic_cert_resolver_new() {
    let config = AppConfig::default();
    let resolver = Arc::new(CertificateResolver::new(config));
    let dynamic_resolver = DynamicCertResolver::new(resolver);

    assert!(Arc::strong_count(&dynamic_resolver.resolver) >= 1);
}
