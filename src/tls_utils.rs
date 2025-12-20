use crate::config::{AppConfig, CertificateConfig};
use anyhow::{Context, Result};
use rustls::server::{ClientHello, ResolvesServerCert, ServerConfig as RustlsServerConfig};
use rustls::sign::CertifiedKey;
use std::collections::HashMap;
use std::io::BufReader;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::fs;

pub struct CertificateResolver {
    config: AppConfig,
    cert_cache: Arc<Mutex<HashMap<String, Arc<CertifiedKey>>>>,
}

impl CertificateResolver {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            cert_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn load_certificate(cert_config: &CertificateConfig) -> Result<Arc<CertifiedKey>> {
        let cert_bytes = fs::read(&cert_config.cert_file).await.with_context(|| {
            format!("Failed to read certificate file: {}", cert_config.cert_file)
        })?;

        let key_bytes = fs::read(&cert_config.key_file)
            .await
            .with_context(|| format!("Failed to read key file: {}", cert_config.key_file))?;

        let mut cert_reader = BufReader::new(cert_bytes.as_slice());
        let certs_bytes =
            rustls_pemfile::certs(&mut cert_reader).context("Failed to parse certificate")?;

        if certs_bytes.is_empty() {
            anyhow::bail!("No certificates found in certificate file");
        }

        let certs: Vec<rustls::Certificate> =
            certs_bytes.into_iter().map(rustls::Certificate).collect();

        let mut key_reader = BufReader::new(key_bytes.as_slice());
        let keys_bytes = rustls_pemfile::pkcs8_private_keys(&mut key_reader)
            .context("Failed to parse private key")?;

        let key_bytes = keys_bytes
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No private key found in key file"))?;

        let key = rustls::PrivateKey(key_bytes);
        let signing_key =
            rustls::sign::any_supported_type(&key).context("Failed to create signing key")?;

        let certified_key = CertifiedKey::new(certs, signing_key);

        Ok(Arc::new(certified_key))
    }

    pub async fn get_cert_for_domain(&self, domain: &str) -> Result<Arc<CertifiedKey>> {
        {
            let cache = self.cert_cache.lock().unwrap();
            if let Some(cert) = cache.get(domain) {
                return Ok(cert.clone());
            }
        }

        let cert_config = self
            .config
            .tls
            .get_cert_config_or_err(domain)
            .with_context(|| format!("No certificate configuration for domain: {}", domain))?;

        let cert = Self::load_certificate(cert_config).await?;

        {
            let mut cache = self.cert_cache.lock().unwrap();
            cache.insert(domain.to_string(), cert.clone());
        }

        Ok(cert)
    }
}

pub struct DynamicCertResolver {
    resolver: Arc<CertificateResolver>,
}

impl DynamicCertResolver {
    pub fn new(resolver: Arc<CertificateResolver>) -> Self {
        Self { resolver }
    }
}

impl ResolvesServerCert for DynamicCertResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let sni = client_hello.server_name()?;

        let resolver = self.resolver.clone();
        let sni_str = sni.to_string();

        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            match handle.block_on(resolver.get_cert_for_domain(&sni_str)) {
                Ok(cert) => Some(cert),
                Err(e) => {
                    tracing::error!("Failed to load certificate for {}: {}", sni_str, e);
                    None
                }
            }
        } else {
            tracing::error!("No tokio runtime available for certificate loading");
            None
        }
    }
}

pub async fn create_server_config(config: &AppConfig) -> Result<RustlsServerConfig> {
    let resolver = Arc::new(CertificateResolver::new(config.clone()));
    let cert_resolver = Arc::new(DynamicCertResolver::new(resolver));

    Ok(RustlsServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_cert_resolver(cert_resolver))
}
