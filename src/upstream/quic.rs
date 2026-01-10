use crate::error::{DnsProxyError, DnsProxyResult};
use crate::quic::client::connect_quic_upstream;
use bytes::Bytes;
use quinn::{Connection, RecvStream, SendStream};
use std::net::SocketAddr;

/// Forward DNS message over QUIC connection
pub async fn forward_quic_dns(connection: &Connection, message: &[u8]) -> DnsProxyResult<Bytes> {
    let (mut send, mut recv) = connection.open_bi().await.map_err(|e| {
        DnsProxyError::Upstream(crate::error::UpstreamError::RequestFailed {
            upstream: connection.remote_address().to_string(),
            reason: format!("Failed to open bidirectional stream: {}", e),
        })
    })?;

    // Send DNS message to upstream
    send.write_all(message).await.map_err(|e| {
        DnsProxyError::Upstream(crate::error::UpstreamError::RequestFailed {
            upstream: connection.remote_address().to_string(),
            reason: format!("Failed to write to upstream: {}", e),
        })
    })?;
    send.finish().map_err(|e| {
        DnsProxyError::Upstream(crate::error::UpstreamError::RequestFailed {
            upstream: connection.remote_address().to_string(),
            reason: format!("Failed to finish upstream stream: {}", e),
        })
    })?;

    // Read response from upstream
    let mut response = Vec::with_capacity(4096);
    loop {
        let mut chunk = vec![0u8; 4096];
        match recv.read(&mut chunk).await {
            Ok(Some(n)) => {
                if n > 0 {
                    response.extend_from_slice(&chunk[..n]);
                } else {
                    break;
                }
            }
            Ok(None) => break,
            Err(e) => {
                return Err(DnsProxyError::Upstream(
                    crate::error::UpstreamError::RequestFailed {
                        upstream: connection.remote_address().to_string(),
                        reason: format!("Failed to read from upstream: {}", e),
                    },
                ));
            }
        }
    }

    Ok(Bytes::from(response))
}

/// Forward DNS message between two QUIC streams (zerocopy where possible)
pub async fn forward_quic_stream(
    mut client_send: SendStream,
    mut client_recv: RecvStream,
    upstream_addr: SocketAddr,
    server_name: &str,
) -> DnsProxyResult<()> {
    // Read DNS message from client
    let mut buffer = Vec::with_capacity(4096);
    loop {
        let mut chunk = vec![0u8; 4096];
        match client_recv.read(&mut chunk).await {
            Ok(Some(n)) => {
                if n > 0 {
                    buffer.extend_from_slice(&chunk[..n]);
                } else {
                    break;
                }
            }
            Ok(None) => break,
            Err(e) => {
                return Err(DnsProxyError::Protocol(format!(
                    "Failed to read from client: {}",
                    e
                )));
            }
        }
    }

    if buffer.is_empty() {
        return Ok(());
    }

    // Connect to upstream
    let upstream_conn = connect_quic_upstream(upstream_addr, server_name).await?;

    // Forward message
    let response = forward_quic_dns(&upstream_conn, &buffer).await?;

    // Send response back to client
    client_send
        .write_all(&response)
        .await
        .map_err(|e| DnsProxyError::Protocol(format!("Failed to write to client: {}", e)))?;
    client_send
        .finish()
        .map_err(|e| DnsProxyError::Protocol(format!("Failed to finish client stream: {}", e)))?;

    Ok(())
}
