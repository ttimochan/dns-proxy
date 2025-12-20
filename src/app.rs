use crate::config::AppConfig;
use crate::rewrite::{SniRewriterType, create_rewriter};
use anyhow::Result;
use tracing::{error, info};

pub struct App {
    config: AppConfig,
    rewriter: SniRewriterType,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let rewriter = create_rewriter(config.rewrite.clone());
        Self { config, rewriter }
    }

    pub fn start(&self) -> Result<()> {
        info!("Starting DNS Proxy Server...");

        self.start_dot_server();
        self.start_doh_server();
        self.start_doq_server();
        self.start_doh3_server();

        info!("All enabled servers started");
        Ok(())
    }

    fn start_dot_server(&self) {
        if !self.config.servers.dot.enabled {
            return;
        }

        use crate::readers::DoTServer;
        let server = DoTServer::new(self.config.clone(), self.rewriter.clone());
        tokio::spawn(async move {
            if let Err(e) = server.start().await {
                error!("DoT server error: {}", e);
            }
        });
        info!("DoT server started");
    }

    fn start_doh_server(&self) {
        if !self.config.servers.doh.enabled {
            return;
        }

        use crate::readers::DoHServer;
        let server = DoHServer::new(self.config.clone(), self.rewriter.clone());
        tokio::spawn(async move {
            if let Err(e) = server.start().await {
                error!("DoH server error: {}", e);
            }
        });
        info!("DoH server started");
    }

    fn start_doq_server(&self) {
        if !self.config.servers.doq.enabled {
            return;
        }

        use crate::readers::DoQServer;
        let server = DoQServer::new(self.config.clone(), self.rewriter.clone());
        tokio::spawn(async move {
            if let Err(e) = server.start().await {
                error!("DoQ server error: {}", e);
            }
        });
        info!("DoQ server started");
    }

    fn start_doh3_server(&self) {
        if !self.config.servers.doh3.enabled {
            return;
        }

        use crate::readers::DoH3Server;
        let server = DoH3Server::new(self.config.clone(), self.rewriter.clone());
        tokio::spawn(async move {
            if let Err(e) = server.start().await {
                error!("DoH3 server error: {}", e);
            }
        });
        info!("DoH3 server started");
    }
}
