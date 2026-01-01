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
fn test_sni_rewrite_error_invalid_hostname() {
    let error = SniRewriteError::InvalidHostname {
        hostname: "".to_string(),
    };
    assert!(error.to_string().contains("Invalid hostname"));
}

#[test]
fn test_sni_rewrite_error_missing_prefix() {
    let error = SniRewriteError::MissingPrefix {
        hostname: "example.com".to_string(),
    };
    assert!(error.to_string().contains("Missing prefix"));
}

#[test]
fn test_sni_rewrite_error_empty_base_domains() {
    let error = SniRewriteError::EmptyBaseDomains;
    assert!(error.to_string().contains("No base domains configured"));
}

#[test]
fn test_sni_rewrite_error_invalid_target_suffix() {
    let error = SniRewriteError::InvalidTargetSuffix {
        suffix: "example.cn".to_string(),
    };
    assert!(error.to_string().contains("Invalid target suffix"));
    assert!(error.to_string().contains("must start with '.'"));
}

#[test]
fn test_certificate_error_file_not_found() {
    let error = CertificateError::FileNotFound {
        path: "/path/to/cert.pem".to_string(),
    };
    assert!(error.to_string().contains("Certificate file not found"));
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
fn test_upstream_error_timeout() {
    let error = UpstreamError::Timeout {
        upstream: "8.8.8.8:853".to_string(),
        timeout: std::time::Duration::from_secs(30),
    };
    assert!(error.to_string().contains("timeout"));
    assert!(error.to_string().contains("8.8.8.8:853"));
}

#[test]
fn test_upstream_error_invalid_address() {
    let error = UpstreamError::InvalidAddress {
        address: "invalid".to_string(),
    };
    assert!(error.to_string().contains("Invalid upstream address"));
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
    let cert_error = CertificateError::FileNotFound {
        path: "/path/to/cert.pem".to_string(),
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
