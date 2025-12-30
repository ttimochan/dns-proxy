/// Trait for rewriting SNI (Server Name Indication) values
///
/// Implementations of this trait extract information from the SNI
/// and rewrite it to a target hostname for upstream forwarding.
#[async_trait::async_trait]
pub trait SniRewriter {
    /// Rewrite the given SNI to a target hostname
    ///
    /// # Arguments
    ///
    /// * `sni` - The original SNI value from the client
    ///
    /// # Returns
    ///
    /// Returns `Some(RewriteResult)` if the SNI was successfully rewritten,
    /// or `None` if the SNI doesn't match any configured pattern.
    async fn rewrite(&self, sni: &str) -> Option<RewriteResult>;
}

/// Result of an SNI rewrite operation
#[derive(Debug, Clone)]
pub struct RewriteResult {
    /// The original SNI value
    pub original: String,
    /// The extracted prefix (e.g., "www" from "www.example.org")
    pub prefix: String,
    /// The target hostname to forward to (e.g., "www.example.cn")
    pub target_hostname: String,
}
