use crate::config::RewriteConfig;
use crate::sni::{RewriteResult, SniRewriter};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub struct BaseSniRewriter {
    config: RewriteConfig,
    pub sni_map: Arc<RwLock<HashMap<String, String>>>,
}

impl BaseSniRewriter {
    pub fn new(config: RewriteConfig) -> Self {
        Self {
            config,
            sni_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn extract_prefix(&self, sni: &str) -> Option<String> {
        for base_domain in &self.config.base_domains {
            if let Some(rest) = sni.strip_suffix(base_domain)
                && !rest.is_empty()
                && rest.ends_with('.')
            {
                let prefix = rest.strip_suffix('.').unwrap_or(rest);
                if !prefix.is_empty() {
                    return Some(prefix.to_string());
                }
            }
        }
        None
    }

    pub fn build_target_hostname(&self, prefix: &str) -> String {
        format!("{}{}", prefix, self.config.target_suffix)
    }
}

#[async_trait::async_trait]
impl SniRewriter for BaseSniRewriter {
    async fn rewrite(&self, sni: &str) -> Option<RewriteResult> {
        let prefix = self.extract_prefix(sni)?;
        let target_hostname = self.build_target_hostname(&prefix);

        // Cache the mapping for future lookups
        {
            let mut cache = self.sni_map.write().await;
            cache.insert(sni.to_string(), target_hostname.clone());
        }

        info!(
            "SNI Rewrite: {} -> Prefix: {} -> Target: {}",
            sni, prefix, target_hostname
        );

        Some(RewriteResult {
            original: sni.to_string(),
            prefix,
            target_hostname,
        })
    }
}

#[async_trait::async_trait]
impl SniRewriter for std::sync::Arc<BaseSniRewriter> {
    async fn rewrite(&self, sni: &str) -> Option<RewriteResult> {
        self.as_ref().rewrite(sni).await
    }
}
