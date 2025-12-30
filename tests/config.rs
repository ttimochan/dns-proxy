use dns_proxy::config::*;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_default_config() {
    let config = AppConfig::default();
    assert_eq!(config.rewrite.base_domains.len(), 2);
    assert!(
        config
            .rewrite
            .base_domains
            .contains(&"example.com".to_string())
    );
    assert!(
        config
            .rewrite
            .base_domains
            .contains(&"example.org".to_string())
    );
    assert_eq!(config.rewrite.target_suffix, ".example.cn");
}

#[test]
fn test_config_from_toml() {
    let toml_content = r#"
[rewrite]
base_domains = ["test.com", "test.org"]
target_suffix = ".test.cn"

[servers.dot]
enabled = true
bind_address = "127.0.0.1"
port = 853

[servers.doh]
enabled = false
bind_address = "0.0.0.0"
port = 443

[servers.doq]
enabled = true
bind_address = "0.0.0.0"
port = 853

[servers.doh3]
enabled = false
bind_address = "0.0.0.0"
port = 443

[upstream]
default = "1.1.1.1:853"
dot = "1.1.1.1:853"
doh = "https://cloudflare-dns.com/dns-query"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(toml_content.as_bytes()).unwrap();
    file.flush().unwrap();

    let config = AppConfig::from_file(file.path()).unwrap();
    assert_eq!(config.rewrite.base_domains.len(), 2);
    assert_eq!(config.rewrite.target_suffix, ".test.cn");
    assert_eq!(config.servers.dot.bind_address, "127.0.0.1");
    assert!(!config.servers.doh.enabled);
}

#[test]
fn test_tls_config_get_cert() {
    let mut tls_config = TlsConfig::default();

    let cert_config = CertificateConfig {
        cert_file: "/path/to/cert.pem".to_string(),
        key_file: "/path/to/key.pem".to_string(),
        ca_file: None,
        require_client_cert: false,
    };

    tls_config
        .certs
        .insert("example.com".to_string(), cert_config.clone());
    tls_config.default = Some(cert_config);

    assert!(tls_config.get_cert_config("example.com").is_some());
    assert!(tls_config.get_cert_config("example.org").is_some());
    assert!(tls_config.get_cert_config("unknown.com").is_some());
}

#[test]
fn test_tls_config_get_cert_or_err() {
    let mut tls_config = TlsConfig::default();

    let cert_config = CertificateConfig {
        cert_file: "/path/to/cert.pem".to_string(),
        key_file: "/path/to/key.pem".to_string(),
        ca_file: None,
        require_client_cert: false,
    };

    tls_config
        .certs
        .insert("example.com".to_string(), cert_config);

    assert!(tls_config.get_cert_config_or_err("example.com").is_ok());
    assert!(tls_config.get_cert_config_or_err("unknown.com").is_err());
}

#[test]
fn test_upstream_config() {
    let config = AppConfig::default();

    let dot_upstream = config.dot_upstream().unwrap();
    assert_eq!(dot_upstream.port(), 853);

    let doq_upstream = config.doq_upstream().unwrap();
    assert_eq!(doq_upstream.port(), 853);
}

#[test]
fn test_load_or_default() {
    let config = AppConfig::load_or_default("/nonexistent/file.toml");
    assert_eq!(config.rewrite.base_domains.len(), 2);
}
