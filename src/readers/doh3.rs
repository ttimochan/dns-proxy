use crate::config::AppConfig;
use crate::rewrite::SniRewriterType;
use anyhow::Result;
use std::sync::Arc;
use tracing::info;

pub struct DoH3Server {
    config: Arc<AppConfig>,
    #[allow(dead_code)]
    rewriter: SniRewriterType,
}

impl DoH3Server {
    pub fn new(config: Arc<AppConfig>, rewriter: SniRewriterType) -> Self {
        Self { config, rewriter }
    }

    pub async fn start(&self) -> Result<()> {
        let server_config = &self.config.servers.doh3;
        if !server_config.enabled {
            info!("DoH3 server is disabled");
            return Ok(());
        }

        anyhow::bail!(
            "DoH3 server requires proper QUIC certificate setup. \
             Please configure certificates or disable DoH3 in config."
        );
    }
}
