use crate::config::AppConfig;
use crate::metrics::Metrics;
use crate::rewrite::{SniRewriterType, create_rewriter};
use anyhow::Result;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info};

/// DNS Proxy application that manages all protocol servers
pub struct App {
    config: Arc<AppConfig>,
    pub rewriter: SniRewriterType,
    pub metrics: Arc<Metrics>,
    handles: Vec<JoinHandle<()>>,
}

impl App {
    /// Create a new App instance with the given configuration
    pub fn new(config: AppConfig) -> Self {
        let config = Arc::new(config);
        let rewriter = create_rewriter(config.rewrite.clone());
        let metrics = Arc::new(Metrics::new());
        Self {
            config,
            rewriter,
            metrics,
            handles: Vec::new(),
        }
    }

    /// Start all enabled servers and return handles for graceful shutdown
    pub fn start(&mut self) -> Result<()> {
        info!("Starting DNS Proxy Server...");

        self.start_healthcheck_server();
        self.start_dot_server();
        self.start_doh_server();
        self.start_doq_server();
        self.start_doh3_server();

        info!("All enabled servers started ({} tasks)", self.handles.len());
        Ok(())
    }

    /// Wait for all server tasks to complete (for graceful shutdown)
    pub async fn wait_for_shutdown(&mut self) {
        info!("Waiting for all servers to shutdown...");
        for handle in self.handles.drain(..) {
            handle.abort();
        }
        info!("All servers shutdown complete");
    }

    fn start_healthcheck_server(&mut self) {
        if !self.config.servers.healthcheck.enabled {
            return;
        }

        use crate::readers::HealthcheckServer;
        let config = Arc::clone(&self.config);
        let metrics = Arc::clone(&self.metrics);
        let handle = tokio::spawn(async move {
            let server = HealthcheckServer::new(config, metrics);
            if let Err(e) = server.start().await {
                error!("Healthcheck server error: {}", e);
            }
        });
        self.handles.push(handle);
        info!(
            "Healthcheck server started on {}:{} at path {}",
            self.config.servers.healthcheck.bind_address,
            self.config.servers.healthcheck.port,
            self.config.servers.healthcheck.path
        );
    }

    fn start_dot_server(&mut self) {
        if !self.config.servers.dot.enabled {
            return;
        }

        use crate::readers::DoTServer;
        let config = Arc::clone(&self.config);
        let rewriter = Arc::clone(&self.rewriter);
        let metrics = Arc::clone(&self.metrics);
        let handle = tokio::spawn(async move {
            let server = DoTServer::new(config, rewriter, metrics);
            if let Err(e) = server.start().await {
                error!("DoT server error: {}", e);
            }
        });
        self.handles.push(handle);
        info!(
            "DoT server started on {}:{}",
            self.config.servers.dot.bind_address, self.config.servers.dot.port
        );
    }

    fn start_doh_server(&mut self) {
        if !self.config.servers.doh.enabled {
            return;
        }

        use crate::readers::DoHServer;
        let config = Arc::clone(&self.config);
        let rewriter = Arc::clone(&self.rewriter);
        let metrics = Arc::clone(&self.metrics);
        let handle = tokio::spawn(async move {
            let server = DoHServer::new(config, rewriter, metrics);
            if let Err(e) = server.start().await {
                error!("DoH server error: {}", e);
            }
        });
        self.handles.push(handle);
        info!(
            "DoH server started on {}:{}",
            self.config.servers.doh.bind_address, self.config.servers.doh.port
        );
    }

    fn start_doq_server(&mut self) {
        if !self.config.servers.doq.enabled {
            return;
        }

        use crate::readers::DoQServer;
        let config = Arc::clone(&self.config);
        let rewriter = Arc::clone(&self.rewriter);
        let metrics = Arc::clone(&self.metrics);
        let handle = tokio::spawn(async move {
            let server = DoQServer::new(config, rewriter, metrics);
            if let Err(e) = server.start().await {
                error!("DoQ server error: {}", e);
            }
        });
        self.handles.push(handle);
        info!(
            "DoQ server started on {}:{}",
            self.config.servers.doq.bind_address, self.config.servers.doq.port
        );
    }

    fn start_doh3_server(&mut self) {
        if !self.config.servers.doh3.enabled {
            return;
        }

        use crate::readers::DoH3Server;
        let config = Arc::clone(&self.config);
        let rewriter = Arc::clone(&self.rewriter);
        let metrics = Arc::clone(&self.metrics);
        let handle = tokio::spawn(async move {
            let server = DoH3Server::new(config, rewriter, metrics);
            if let Err(e) = server.start().await {
                error!("DoH3 server error: {}", e);
            }
        });
        self.handles.push(handle);
        info!(
            "DoH3 server started on {}:{}",
            self.config.servers.doh3.bind_address, self.config.servers.doh3.port
        );
    }
}
