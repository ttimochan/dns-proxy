pub trait SniRewriter {
    async fn rewrite(&self, sni: &str) -> Option<RewriteResult>;
}

#[derive(Debug, Clone)]
pub struct RewriteResult {
    pub original: String,
    pub prefix: String,
    pub target_hostname: String,
}
