use dns_proxy::error::{CertificateError, DnsProxyError, SniRewriteError, UpstreamError};

#[test]
fn test_sni_rewrite_error_no_matching_base_domain() {
    let error = SniRewriteError::NoMatchingBaseDomain {
        hostname: "example.com".to_string(),
    };
    assert!(error.to_string().contains("No matching base domain"));
    assert!(error.to_string().contains("example.com"));
}

#[test]
fn test_certificate_error_load_failed() {
    let error = CertificateError::LoadFailed {
        path: "/path/to/cert.pem".to_string(),
        reason: "Permission denied".to_string(),
    };
    assert!(error.to_string().contains("Failed to load certificate"));
    assert!(error.to_string().contains("/path/to/cert.pem"));
}

#[test]
fn test_certificate_error_not_configured() {
    let error = CertificateError::NotConfigured {
        domain: "example.com".to_string(),
    };
    assert!(error.to_string().contains("No certificate configured"));
    assert!(error.to_string().contains("example.com"));
}

#[test]
fn test_upstream_error_connection_failed() {
    let error = UpstreamError::ConnectionFailed {
        upstream: "8.8.8.8:853".to_string(),
        reason: "Connection refused".to_string(),
    };
    assert!(error.to_string().contains("Failed to connect"));
    assert!(error.to_string().contains("8.8.8.8:853"));
}

#[test]
fn test_upstream_error_request_failed() {
    let error = UpstreamError::RequestFailed {
        upstream: "8.8.8.8:853".to_string(),
        reason: "Timeout".to_string(),
    };
    assert!(error.to_string().contains("Upstream request failed"));
    assert!(error.to_string().contains("8.8.8.8:853"));
}

#[test]
fn test_dns_proxy_error_from_sni_rewrite() {
    let sni_error = SniRewriteError::NoMatchingBaseDomain {
        hostname: "test.com".to_string(),
    };
    let dns_error: DnsProxyError = sni_error.into();
    assert!(matches!(dns_error, DnsProxyError::SniRewrite(_)));
}

#[test]
fn test_dns_proxy_error_from_certificate() {
    let cert_error = CertificateError::LoadFailed {
        path: "/path/to/cert.pem".to_string(),
        reason: "Parse error".to_string(),
    };
    let dns_error: DnsProxyError = cert_error.into();
    assert!(matches!(dns_error, DnsProxyError::Certificate(_)));
}

#[test]
fn test_dns_proxy_error_from_upstream() {
    let upstream_error = UpstreamError::ConnectionFailed {
        upstream: "8.8.8.8:853".to_string(),
        reason: "Connection refused".to_string(),
    };
    let dns_error: DnsProxyError = upstream_error.into();
    assert!(matches!(dns_error, DnsProxyError::Upstream(_)));
}

#[test]
fn test_dns_proxy_error_from_io() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let dns_error: DnsProxyError = io_error.into();
    assert!(matches!(dns_error, DnsProxyError::Io(_)));
}
