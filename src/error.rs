/// Error types for DNS Proxy
use thiserror::Error;

/// Main error type for DNS Proxy operations
#[derive(Error, Debug)]
pub enum DnsProxyError {
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// SNI rewrite errors
    #[error("SNI rewrite failed: {0}")]
    SniRewrite(#[from] SniRewriteError),

    /// TLS/SSL errors
    #[error("TLS error: {0}")]
    Tls(String),

    /// Certificate errors
    #[error("Certificate error: {0}")]
    Certificate(#[from] CertificateError),

    /// Upstream connection errors
    #[error("Upstream connection error: {0}")]
    Upstream(#[from] UpstreamError),

    /// Network I/O errors
    #[error("Network I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Protocol-specific errors
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Timeout errors
    #[error("Operation timeout: {0}")]
    Timeout(String),

    /// Invalid input errors
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// SNI rewrite specific errors
#[derive(Error, Debug)]
pub enum SniRewriteError {
    /// No matching base domain found
    #[error("No matching base domain found for hostname: {hostname}")]
    NoMatchingBaseDomain { hostname: String },

    /// Invalid hostname format
    #[error("Invalid hostname format: {hostname}")]
    InvalidHostname { hostname: String },

    /// Missing prefix in hostname
    #[error("Missing prefix in hostname: {hostname} (expected format: prefix.base_domain)")]
    MissingPrefix { hostname: String },

    /// Empty base domain list
    #[error("No base domains configured for SNI rewriting")]
    EmptyBaseDomains,

    /// Invalid target suffix
    #[error("Invalid target suffix: {suffix} (must start with '.')")]
    InvalidTargetSuffix { suffix: String },
}

/// Certificate-related errors
#[derive(Error, Debug)]
pub enum CertificateError {
    /// Certificate file not found
    #[error("Certificate file not found: {path}")]
    FileNotFound { path: String },

    /// Failed to load certificate
    #[error("Failed to load certificate from {path}: {reason}")]
    LoadFailed { path: String, reason: String },

    /// Certificate not configured for domain
    #[error("No certificate configured for domain: {domain}")]
    NotConfigured { domain: String },

    /// Invalid certificate format
    #[error("Invalid certificate format: {reason}")]
    InvalidFormat { reason: String },

    /// Private key error
    #[error("Private key error: {reason}")]
    PrivateKey { reason: String },
}

/// Upstream connection errors
#[derive(Error, Debug)]
pub enum UpstreamError {
    /// Connection failed
    #[error("Failed to connect to upstream {upstream}: {reason}")]
    ConnectionFailed { upstream: String, reason: String },

    /// Request failed
    #[error("Upstream request failed to {upstream}: {reason}")]
    RequestFailed { upstream: String, reason: String },

    /// Timeout
    #[error("Upstream request timeout to {upstream} after {timeout:?}")]
    Timeout {
        upstream: String,
        timeout: std::time::Duration,
    },

    /// Invalid upstream address
    #[error("Invalid upstream address: {address}")]
    InvalidAddress { address: String },

    /// Upstream returned error status
    #[error("Upstream returned error status {status} from {upstream}")]
    ErrorStatus { upstream: String, status: u16 },
}

/// Result type alias for convenience
pub type DnsProxyResult<T> = Result<T, DnsProxyError>;

/// Helper trait to convert errors to [`DnsProxyError`].
///
/// This trait provides a convenient way to wrap errors from external
/// operations (e.g., parsing, validation) into [`DnsProxyError::InvalidInput`]
/// with additional context.
///
/// # Example
///
/// ```rust
/// use dns_proxy::error::ToDnsProxyError;
///
/// let result: Result<String, &'static str> = Err("invalid format");
/// let converted = result.to_dns_proxy_error("JSON parsing").unwrap_err();
/// ```
pub trait ToDnsProxyError<T> {
    /// Convert the result to a [`DnsProxyResult`] with context.
    ///
    /// # Arguments
    ///
    /// * `context` - Additional context message describing where/why the error occurred
    ///
    /// # Returns
    ///
    /// [`DnsProxyResult`] with the original value or wrapped error
    fn to_dns_proxy_error(self, context: &str) -> Result<T, DnsProxyError>;
}

impl<T, E: std::fmt::Display> ToDnsProxyError<T> for Result<T, E> {
    fn to_dns_proxy_error(self, context: &str) -> Result<T, DnsProxyError> {
        self.map_err(|e| DnsProxyError::InvalidInput(format!("{}: {}", context, e)))
    }
}
