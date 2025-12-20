use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub rewrite: RewriteConfig,
    pub servers: ServersConfig,
    pub upstream: UpstreamConfig,
    #[serde(default)]
    pub tls: TlsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteConfig {
    /// Base domains to match (e.g., ["example.com", "example.org"])
    /// The rewriter will extract prefix from hostnames matching these base domains
    pub base_domains: Vec<String>,
    /// Target suffix for upstream (e.g., ".example.cn")
    /// The extracted prefix will be combined with this suffix to form the target hostname
    pub target_suffix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServersConfig {
    pub dot: ServerPortConfig,
    pub doh: ServerPortConfig,
    pub doq: ServerPortConfig,
    pub doh3: ServerPortConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPortConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    pub default: String,
    pub dot: Option<String>,
    pub doh: Option<String>,
    pub doq: Option<String>,
    pub doh3: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Default certificate configuration (used when no domain-specific cert is found)
    #[serde(default)]
    pub default: Option<CertificateConfig>,
    /// Domain-specific certificate configurations
    /// Key is the domain name (e.g., "example.com"), value is the certificate config
    #[serde(default)]
    pub certs: std::collections::HashMap<String, CertificateConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateConfig {
    /// Certificate file path (PEM format)
    pub cert_file: String,
    /// Private key file path (PEM format)
    pub key_file: String,
    /// CA certificate file path for client verification (optional)
    pub ca_file: Option<String>,
    /// Whether to require client certificate
    #[serde(default)]
    pub require_client_cert: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            default: None,
            certs: std::collections::HashMap::new(),
        }
    }
}

impl TlsConfig {
    /// Get certificate configuration for a specific domain
    /// Returns domain-specific cert if exists, otherwise returns default cert
    pub fn get_cert_config(&self, domain: &str) -> Option<&CertificateConfig> {
        self.certs.get(domain).or(self.default.as_ref())
    }

    /// Get certificate configuration for a specific domain, or return error if not found
    pub fn get_cert_config_or_err(&self, domain: &str) -> Result<&CertificateConfig> {
        self.get_cert_config(domain).ok_or_else(|| {
            anyhow::anyhow!("No certificate configuration found for domain: {}", domain)
        })
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            rewrite: RewriteConfig {
                base_domains: vec!["example.com".to_string(), "example.org".to_string()],
                target_suffix: ".example.cn".to_string(),
            },
            servers: ServersConfig {
                dot: ServerPortConfig {
                    enabled: true,
                    bind_address: "0.0.0.0".to_string(),
                    port: 853,
                },
                doh: ServerPortConfig {
                    enabled: true,
                    bind_address: "0.0.0.0".to_string(),
                    port: 443,
                },
                doq: ServerPortConfig {
                    enabled: true,
                    bind_address: "0.0.0.0".to_string(),
                    port: 853,
                },
                doh3: ServerPortConfig {
                    enabled: false,
                    bind_address: "0.0.0.0".to_string(),
                    port: 443,
                },
            },
            upstream: UpstreamConfig {
                default: "8.8.8.8:853".to_string(),
                dot: Some("8.8.8.8:853".to_string()),
                doh: Some("https://dns.google/dns-query".to_string()),
                doq: Some("8.8.8.8:853".to_string()),
                doh3: Some("https://dns.google/dns-query".to_string()),
            },
            tls: TlsConfig::default(),
        }
    }
}

impl AppConfig {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;
        let config: AppConfig =
            toml::from_str(&content).with_context(|| "Failed to parse config file")?;
        Ok(config)
    }

    /// Load configuration from file or use default
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        Self::from_file(path).unwrap_or_else(|e| {
            tracing::warn!("Failed to load config file, using defaults: {}", e);
            Self::default()
        })
    }

    /// Get upstream address for DoT
    pub fn dot_upstream(&self) -> SocketAddr {
        self.upstream
            .dot
            .as_ref()
            .or_else(|| Some(&self.upstream.default))
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| "8.8.8.8:853".parse().unwrap())
    }

    /// Get upstream URL for DoH
    pub fn doh_upstream(&self) -> &str {
        self.upstream
            .doh
            .as_deref()
            .unwrap_or("https://dns.google/dns-query")
    }

    /// Get upstream address for DoQ
    pub fn doq_upstream(&self) -> SocketAddr {
        self.upstream
            .doq
            .as_ref()
            .or_else(|| Some(&self.upstream.default))
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| "8.8.8.8:853".parse().unwrap())
    }
}
