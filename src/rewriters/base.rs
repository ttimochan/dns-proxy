use crate::config::RewriteConfig;
use crate::sni::{RewriteResult, SniRewriter};
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{info, warn};

pub struct BaseSniRewriter {
    config: RewriteConfig,
    pub sni_map: Arc<DashMap<String, String>>,
}

impl BaseSniRewriter {
    pub fn new(config: RewriteConfig) -> Self {
        Self {
            config,
            sni_map: Arc::new(DashMap::new()),
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
        // Validate input
        if sni.is_empty() {
            warn!("Empty SNI provided for rewrite");
            return None;
        }

        // Check if base domains are configured
        if self.config.base_domains.is_empty() {
            warn!("No base domains configured for SNI rewriting");
            return None;
        }

        // Validate target suffix
        if !self.config.target_suffix.starts_with('.') {
            warn!(
                "Invalid target suffix: {} (must start with '.')",
                self.config.target_suffix
            );
            return None;
        }

        // Try to extract prefix
        let prefix = match self.extract_prefix(sni) {
            Some(p) => p,
            None => {
                // Handle rewrite failure based on strategy
                match self.config.rewrite_failure_strategy.as_str() {
                    "passthrough" => {
                        warn!(
                            "SNI rewrite failed for '{}', using passthrough strategy",
                            sni
                        );
                        // Return result with original hostname as target
                        return Some(RewriteResult {
                            original: sni.to_string(),
                            prefix: String::new(),
                            target_hostname: sni.to_string(),
                        });
                    }
                    _ => {
                        // Default: return None (error strategy)
                        return None;
                    }
                }
            }
        };

        let target_hostname = self.build_target_hostname(&prefix);

        // Cache the mapping for future lookups (lock-free with DashMap)
        self.sni_map
            .insert(sni.to_string(), target_hostname.clone());

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
