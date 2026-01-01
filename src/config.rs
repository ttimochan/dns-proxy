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
    #[serde(default)]
    pub logging: LoggingConfig,
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
    #[serde(default = "HealthcheckConfig::default")]
    pub healthcheck: HealthcheckConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPortConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthcheckConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub port: u16,
    pub path: String,
}

impl Default for HealthcheckConfig {
    fn default() -> Self {
        HealthcheckConfig {
            enabled: true,
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            path: "/health".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    pub default: String,
    pub dot: Option<String>,
    pub doh: Option<String>,
    pub doq: Option<String>,
    pub doh3: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
pub struct LoggingConfig {
    /// Log level: trace, debug, info, warn, error (default: info)
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Log file path (optional, if not set, logs only to stdout/stderr)
    #[serde(default)]
    pub file: Option<String>,
    /// Enable JSON format for logs (default: false)
    #[serde(default)]
    pub json: bool,
    /// Enable log rotation (default: true if file is set)
    #[serde(default = "default_true")]
    pub rotation: bool,
    /// Maximum log file size in bytes before rotation (default: 10MB)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    /// Number of log files to keep (default: 5)
    #[serde(default = "default_max_files")]
    pub max_files: usize,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_true() -> bool {
    true
}

fn default_max_file_size() -> u64 {
    10 * 1024 * 1024 // 10MB
}

fn default_max_files() -> usize {
    5
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: None,
            json: false,
            rotation: default_true(),
            max_file_size: default_max_file_size(),
            max_files: default_max_files(),
        }
    }
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
                healthcheck: HealthcheckConfig::default(),
            },
            upstream: UpstreamConfig {
                default: "8.8.8.8:853".to_string(),
                dot: Some("8.8.8.8:853".to_string()),
                doh: Some("https://dns.google/dns-query".to_string()),
                doq: Some("8.8.8.8:853".to_string()),
                doh3: Some("https://dns.google/dns-query".to_string()),
            },
            tls: TlsConfig::default(),
            logging: LoggingConfig::default(),
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
    /// Returns the configured DoT upstream or default upstream as SocketAddr
    pub fn dot_upstream(&self) -> Result<SocketAddr> {
        self.upstream
            .dot
            .as_deref()
            .or(Some(self.upstream.default.as_str()))
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid upstream address for DoT: {:?} or default: {}",
                    self.upstream.dot,
                    self.upstream.default
                )
            })
    }

    /// Get upstream address for DoQ
    /// Returns the configured DoQ upstream or default upstream as SocketAddr
    pub fn doq_upstream(&self) -> Result<SocketAddr> {
        self.upstream
            .doq
            .as_deref()
            .or(Some(self.upstream.default.as_str()))
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid upstream address for DoQ: {:?} or default: {}",
                    self.upstream.doq,
                    self.upstream.default
                )
            })
    }

    /// Get upstream hostname for DoT/DoQ (extracted from address or default)
    /// This is used for SNI in TLS connections
    pub fn dot_upstream_hostname(&self) -> String {
        // Try to extract hostname from configured upstream
        if let Some(addr) = &self.upstream.dot {
            if let Ok(parsed) = addr.parse::<SocketAddr>() {
                return parsed.ip().to_string();
            }
            // If not a SocketAddr, try to extract hostname from URL-like string
            if let Some(host) = addr.split(':').next() {
                return host.to_string();
            }
        }
        // Fallback to default
        if let Ok(parsed) = self.upstream.default.parse::<SocketAddr>() {
            parsed.ip().to_string()
        } else {
            self.upstream
                .default
                .split(':')
                .next()
                .unwrap_or("dns.google")
                .to_string()
        }
    }

    /// Validate configuration before starting servers
    pub fn validate(&self) -> Result<()> {
        use std::collections::HashSet;

        // Check for port conflicts
        let mut ports = HashSet::new();

        // Check standard server ports
        let standard_servers: &[(&str, &ServerPortConfig)] = &[
            ("dot", &self.servers.dot),
            ("doh", &self.servers.doh),
            ("doq", &self.servers.doq),
            ("doh3", &self.servers.doh3),
        ];

        for (name, config) in standard_servers {
            if config.enabled {
                let addr = format!("{}:{}", config.bind_address, config.port);
                if let Ok(socket_addr) = addr.parse::<SocketAddr>() {
                    if !ports.insert((socket_addr.ip(), socket_addr.port())) {
                        anyhow::bail!(
                            "Port conflict: {} is already used by another server",
                            socket_addr.port()
                        );
                    }
                } else {
                    anyhow::bail!("Invalid bind address for {}: {}", name, addr);
                }
            }
        }

        // Check healthcheck server port
        if self.servers.healthcheck.enabled {
            let addr = format!(
                "{}:{}",
                self.servers.healthcheck.bind_address, self.servers.healthcheck.port
            );
            if let Ok(socket_addr) = addr.parse::<SocketAddr>() {
                if !ports.insert((socket_addr.ip(), socket_addr.port())) {
                    anyhow::bail!(
                        "Port conflict: {} is already used by another server",
                        socket_addr.port()
                    );
                }
            } else {
                anyhow::bail!("Invalid bind address for healthcheck: {}", addr);
            }
        }

        // Validate TLS certificate files exist
        if let Some(default_cert) = &self.tls.default {
            std::fs::metadata(&default_cert.cert_file).with_context(|| {
                format!(
                    "Default certificate file not found: {}",
                    default_cert.cert_file
                )
            })?;
            std::fs::metadata(&default_cert.key_file).with_context(|| {
                format!("Default key file not found: {}", default_cert.key_file)
            })?;
        }

        for (domain, cert_config) in &self.tls.certs {
            std::fs::metadata(&cert_config.cert_file).with_context(|| {
                format!(
                    "Certificate file not found for {}: {}",
                    domain, cert_config.cert_file
                )
            })?;
            std::fs::metadata(&cert_config.key_file).with_context(|| {
                format!(
                    "Key file not found for {}: {}",
                    domain, cert_config.key_file
                )
            })?;
        }

        // Validate rewrite configuration
        if self.rewrite.base_domains.is_empty() {
            anyhow::bail!("At least one base domain must be configured for SNI rewriting");
        }

        if !self.rewrite.target_suffix.starts_with('.') {
            anyhow::bail!("Target suffix must start with '.' (e.g., '.example.cn')");
        }

        Ok(())
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
