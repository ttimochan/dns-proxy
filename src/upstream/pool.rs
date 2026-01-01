use dashmap::DashMap;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// Default keepalive timeout (60 seconds)
const DEFAULT_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(60);

/// Default connection timeout (10 seconds)
const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);

/// Default max idle connections per SNI
const DEFAULT_MAX_IDLE_CONNECTIONS: usize = 10;

/// HTTP client type with HTTPS support
pub type HttpClient = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;

/// Connection pool manager that maintains separate HTTP clients for each SNI
/// This allows connection reuse and keepalive for the same target hostname
pub struct ConnectionPool {
    /// Map from SNI (target hostname) to HTTP client
    clients: Arc<DashMap<String, Arc<HttpClient>>>,
    /// Keepalive timeout duration
    keepalive_timeout: Duration,
    /// Connection timeout duration
    connection_timeout: Duration,
    /// Max idle connections per SNI
    max_idle_connections: usize,
}

impl ConnectionPool {
    /// Create a new connection pool with default settings
    pub fn new() -> Self {
        Self::with_config(
            DEFAULT_KEEPALIVE_TIMEOUT,
            DEFAULT_CONNECTION_TIMEOUT,
            DEFAULT_MAX_IDLE_CONNECTIONS,
        )
    }

    /// Create a new connection pool with custom configuration
    pub fn with_config(
        keepalive_timeout: Duration,
        connection_timeout: Duration,
        max_idle_connections: usize,
    ) -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            keepalive_timeout,
            connection_timeout,
            max_idle_connections,
        }
    }

    /// Get or create an HTTP client for the given SNI (target hostname)
    /// This ensures that requests to the same SNI reuse connections
    pub fn get_client(&self, sni: &str) -> Arc<HttpClient> {
        // Fast path: check if client already exists
        if let Some(client) = self.clients.get(sni) {
            debug!("Reusing existing HTTP client for SNI: {}", sni);
            return Arc::clone(client.value());
        }

        // Slow path: create new client for this SNI
        debug!("Creating new HTTP client for SNI: {}", sni);
        let client = self.create_client();
        let client_arc = Arc::new(client);

        // Insert into map (may race with another thread, but that's okay)
        // We'll use the first one that gets inserted
        self.clients
            .entry(sni.to_string())
            .or_insert_with(|| Arc::clone(&client_arc));

        // Return the client from the map (could be ours or another thread's)
        self.clients
            .get(sni)
            .map(|entry| Arc::clone(entry.value()))
            .unwrap_or(client_arc)
    }

    /// Create a new HTTP client with HTTPS support and keepalive configuration
    fn create_client(&self) -> HttpClient {
        // Create HTTP connector with keepalive settings
        let mut http_connector = HttpConnector::new();
        http_connector.set_keepalive(Some(self.keepalive_timeout));
        http_connector.set_connect_timeout(Some(self.connection_timeout));

        // Create HTTPS connector with rustls
        // HttpsConnectorBuilder::new() returns a Result
        let https_connector = HttpsConnectorBuilder::new()
            .with_native_roots()
            .expect("Failed to load native root certificates")
            .https_or_http()
            .enable_http2()
            .wrap_connector(http_connector);

        // Build HTTP client with connection pool settings
        Client::builder(TokioExecutor::new())
            .pool_max_idle_per_host(self.max_idle_connections)
            .pool_idle_timeout(self.keepalive_timeout)
            .set_host(false) // Don't set Host header automatically, we'll do it manually
            .build(https_connector)
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}
