/// Common server startup utilities
use crate::config::{AppConfig, ServerPortConfig};
use crate::metrics::Metrics;
use crate::rewrite::SniRewriterType;
use anyhow::Result;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info};

/// Common server startup helper
pub struct ServerStarter;

impl ServerStarter {
    /// Start a server with a closure that receives cloned resources
    pub fn start_server<F, Fut>(
        name: &str,
        config: &ServerPortConfig,
        resources: ServerResources,
        server_future: F,
    ) -> Option<JoinHandle<()>>
    where
        F: FnOnce(ServerResources) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        if !config.enabled {
            info!("{} server is disabled", name);
            return None;
        }

        let bind_addr = format!("{}:{}", config.bind_address, config.port);
        let name_for_log = name.to_string(); // For final log message
        let name = name.to_string(); // Convert to owned String for 'static lifetime
        let handle = tokio::spawn(async move {
            if let Err(e) = server_future(resources).await {
                error!("{} server error: {}", name, e);
            }
        });

        info!("{} server started on {}", name_for_log, bind_addr);
        Some(handle)
    }
}

/// Common resources shared across servers
#[derive(Clone)]
pub struct ServerResources {
    pub config: Arc<AppConfig>,
    pub rewriter: SniRewriterType,
    pub metrics: Arc<Metrics>,
}

impl ServerResources {
    pub fn new(config: Arc<AppConfig>, rewriter: SniRewriterType, metrics: Arc<Metrics>) -> Self {
        Self {
            config,
            rewriter,
            metrics,
        }
    }
}
