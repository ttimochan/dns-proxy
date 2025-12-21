use anyhow::{Context, Result};
use quinn::crypto::rustls::QuicClientConfig;
use quinn::rustls::pki_types::CertificateDer;
use quinn::rustls::{ClientConfig, RootCertStore};
use quinn::{ClientConfig as QuinnClientConfig, Connection, Endpoint};
use std::net::SocketAddr;
use std::sync::Arc;

/// Create a QUIC client connection to upstream server
pub async fn connect_quic_upstream(addr: SocketAddr, server_name: &str) -> Result<Connection> {
    // Create client TLS config with native root certificates
    let mut root_store = RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs()? {
        root_store.add(CertificateDer::from(cert.0))?;
    }

    let client_crypto = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let quic_client_config =
        QuicClientConfig::try_from(client_crypto).context("Failed to create QuicClientConfig")?;
    let client_config = QuinnClientConfig::new(Arc::new(quic_client_config));

    let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
    endpoint.set_default_client_config(client_config);

    endpoint
        .connect(addr, server_name)?
        .await
        .context("Failed to connect to upstream QUIC server")
}
