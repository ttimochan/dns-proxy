use crate::config::RewriteConfig;
use crate::rewriters::BaseSniRewriter;
use std::sync::Arc;

/// Type alias for the SNI rewriter used throughout the application
pub type SniRewriterType = Arc<BaseSniRewriter>;

/// Create a new SNI rewriter instance from the given configuration
///
/// # Arguments
///
/// * `config` - The rewrite configuration containing base domains and target suffix
///
/// # Returns
///
/// Returns an `Arc`-wrapped `BaseSniRewriter` that can be shared across tasks
pub fn create_rewriter(config: RewriteConfig) -> SniRewriterType {
    Arc::new(BaseSniRewriter::new(config))
}
