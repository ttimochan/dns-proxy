use crate::config::AppConfig;
use crate::rewrite::{SniRewriterType, create_rewriter};
use anyhow::Result;
use std::sync::Arc;
use tracing::{error, info};

pub struct App {
    config: Arc<AppConfig>,
    pub rewriter: SniRewriterType,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let config = Arc::new(config);
        let rewriter = create_rewriter(config.rewrite.clone());
        Self { config, rewriter }
    }

    pub fn start(&self) -> Result<()> {
        info!("Starting DNS Proxy Server...");

        self.start_healthcheck_server();
        self.start_dot_server();
        self.start_doh_server();
        self.start_doq_server();
        self.start_doh3_server();

        info!("All enabled servers started");
        Ok(())
    }

    fn start_healthcheck_server(&self) {
        if !self.config.servers.healthcheck.enabled {
            return;
        }

        use crate::readers::HealthcheckServer;
        let config = Arc::clone(&self.config);
        tokio::spawn(async move {
            let server = HealthcheckServer::new(config);
            if let Err(e) = server.start().await {
                error!("Healthcheck server error: {}", e);
            }
        });
        info!(
            "Healthcheck server started on {}:{} at path {}",
            self.config.servers.healthcheck.bind_address,
            self.config.servers.healthcheck.port,
            self.config.servers.healthcheck.path
        );
    }

    fn start_dot_server(&self) {
        if !self.config.servers.dot.enabled {
            return;
        }

        use crate::readers::DoTServer;
        let config = Arc::clone(&self.config);
        let rewriter = Arc::clone(&self.rewriter);
        tokio::spawn(async move {
            let server = DoTServer::new(config, rewriter);
            if let Err(e) = server.start().await {
                error!("DoT server error: {}", e);
            }
        });
        info!(
            "DoT server started on {}:{}",
            self.config.servers.dot.bind_address, self.config.servers.dot.port
        );
    }

    fn start_doh_server(&self) {
        if !self.config.servers.doh.enabled {
            return;
        }

        use crate::readers::DoHServer;
        let config = Arc::clone(&self.config);
        let rewriter = Arc::clone(&self.rewriter);
        tokio::spawn(async move {
            let server = DoHServer::new(config, rewriter);
            if let Err(e) = server.start().await {
                error!("DoH server error: {}", e);
            }
        });
        info!(
            "DoH server started on {}:{}",
            self.config.servers.doh.bind_address, self.config.servers.doh.port
        );
    }

    fn start_doq_server(&self) {
        if !self.config.servers.doq.enabled {
            return;
        }

        use crate::readers::DoQServer;
        let config = Arc::clone(&self.config);
        let rewriter = Arc::clone(&self.rewriter);
        tokio::spawn(async move {
            let server = DoQServer::new(config, rewriter);
            if let Err(e) = server.start().await {
                error!("DoQ server error: {}", e);
            }
        });
        info!(
            "DoQ server started on {}:{}",
            self.config.servers.doq.bind_address, self.config.servers.doq.port
        );
    }

    fn start_doh3_server(&self) {
        if !self.config.servers.doh3.enabled {
            return;
        }

        use crate::readers::DoH3Server;
        let config = Arc::clone(&self.config);
        let rewriter = Arc::clone(&self.rewriter);
        tokio::spawn(async move {
            let server = DoH3Server::new(config, rewriter);
            if let Err(e) = server.start().await {
                error!("DoH3 server error: {}", e);
            }
        });
        info!(
            "DoH3 server started on {}:{}",
            self.config.servers.doh3.bind_address, self.config.servers.doh3.port
        );
    }
}
