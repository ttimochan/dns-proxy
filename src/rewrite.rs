use crate::config::RewriteConfig;
use crate::rewriters::BaseSniRewriter;
use std::sync::Arc;

pub type SniRewriterType = Arc<BaseSniRewriter>;

pub fn create_rewriter(config: RewriteConfig) -> SniRewriterType {
    Arc::new(BaseSniRewriter::new(config))
}
