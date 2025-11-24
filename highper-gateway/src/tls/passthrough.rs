//! TLS Passthrough Support
//!
//! Routes TLS connections based on SNI (Server Name Indication) without terminating TLS.
//! The proxy acts as a TCP proxy, forwarding encrypted traffic to backends.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::{debug, error, info};
use anyhow::{Result, anyhow};

/// SNI (Server Name Indication) information extracted from TLS ClientHello
#[derive(Debug, Clone)]
pub struct SniInfo {
    /// Server name from SNI extension
    pub server_name: String,
    /// Full ClientHello message (to be replayed to backend)
    pub client_hello: Vec<u8>,
}

/// Extract SNI from TLS ClientHello message
pub async fn extract_sni<R>(stream: &mut R) -> Result<SniInfo>
where
    R: AsyncRead + Unpin,
{
    // Read TLS record header (5 bytes)
    let mut header = [0u8; 5];
    stream.read_exact(&mut header).await?;

    // Check if this is a TLS handshake (content type = 0x16)
    if header[0] != 0x16 {
        return Err(anyhow!("Not a TLS handshake"));
    }

    // Get TLS record length (bytes 3-4, big-endian)
    let record_length = u16::from_be_bytes([header[3], header[4]]) as usize;

    // Read the full TLS record
    let mut record = vec![0u8; record_length];
    stream.read_exact(&mut record).await?;

    // Parse ClientHello
    let server_name = parse_client_hello(&record)?;

    // Combine header + record for replaying to backend
    let mut client_hello = Vec::with_capacity(5 + record_length);
    client_hello.extend_from_slice(&header);
    client_hello.extend_from_slice(&record);

    Ok(SniInfo {
        server_name,
        client_hello,
    })
}

/// Parse TLS ClientHello to extract SNI
fn parse_client_hello(data: &[u8]) -> Result<String> {
    if data.len() < 4 {
        return Err(anyhow!("ClientHello too short"));
    }

    // Check handshake type (should be 0x01 for ClientHello)
    if data[0] != 0x01 {
        return Err(anyhow!("Not a ClientHello"));
    }

    // Skip handshake length (3 bytes)
    let mut offset = 4;

    // Skip client version (2 bytes)
    offset += 2;

    // Skip client random (32 bytes)
    offset += 32;

    if offset >= data.len() {
        return Err(anyhow!("ClientHello truncated"));
    }

    // Skip session ID
    let session_id_len = data[offset] as usize;
    offset += 1 + session_id_len;

    if offset + 2 >= data.len() {
        return Err(anyhow!("ClientHello truncated"));
    }

    // Skip cipher suites
    let cipher_suites_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
    offset += 2 + cipher_suites_len;

    if offset + 1 >= data.len() {
        return Err(anyhow!("ClientHello truncated"));
    }

    // Skip compression methods
    let compression_methods_len = data[offset] as usize;
    offset += 1 + compression_methods_len;

    if offset + 2 >= data.len() {
        // No extensions
        return Err(anyhow!("No SNI extension found"));
    }

    // Parse extensions
    let extensions_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
    offset += 2;

    let extensions_end = offset + extensions_len;

    // Find SNI extension (type = 0x0000)
    while offset + 4 <= extensions_end {
        let ext_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let ext_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;

        if ext_type == 0x0000 {
            // Found SNI extension
            return parse_sni_extension(&data[offset..offset + ext_len]);
        }

        offset += ext_len;
    }

    Err(anyhow!("No SNI extension found"))
}

/// Parse SNI extension to extract server name
fn parse_sni_extension(data: &[u8]) -> Result<String> {
    if data.len() < 5 {
        return Err(anyhow!("SNI extension too short"));
    }

    // Skip server name list length (2 bytes)
    let mut offset = 2;

    // Read server name type (should be 0x00 for hostname)
    if data[offset] != 0x00 {
        return Err(anyhow!("Invalid SNI name type"));
    }
    offset += 1;

    // Read server name length
    let name_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
    offset += 2;

    if offset + name_len > data.len() {
        return Err(anyhow!("SNI name truncated"));
    }

    // Extract server name
    let server_name = String::from_utf8(data[offset..offset + name_len].to_vec())?;

    Ok(server_name)
}

/// Passthrough TLS connection to backend
pub async fn passthrough_tls<C, B>(
    mut client: C,
    mut backend: B,
    client_hello: Vec<u8>,
) -> Result<()>
where
    C: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    // First, replay the ClientHello to the backend
    backend.write_all(&client_hello).await?;
    backend.flush().await?;

    debug!("TLS ClientHello replayed to backend");

    // Now do bidirectional copy for the rest of the connection
    match tokio::io::copy_bidirectional(&mut client, &mut backend).await {
        Ok((client_to_server, server_to_client)) => {
            info!(
                "TLS passthrough connection closed. Transferred: client→server: {} bytes, server→client: {} bytes",
                client_to_server,
                server_to_client
            );
            Ok(())
        }
        Err(e) => {
            error!("TLS passthrough error: {}", e);
            Err(anyhow!("TLS passthrough error: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sni_extension() {
        // SNI extension with hostname "example.com"
        let sni_data = vec![
            0x00, 0x0f, // Server name list length (15 bytes)
            0x00,       // Name type (0 = hostname)
            0x00, 0x0b, // Name length (11 bytes)
            0x65, 0x78, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x2e, 0x63, 0x6f, 0x6d, // "example.com"
        ];

        let result = parse_sni_extension(&sni_data).unwrap();
        assert_eq!(result, "example.com");
    }

    #[test]
    fn test_parse_sni_extension_invalid() {
        let sni_data = vec![0x00]; // Too short
        assert!(parse_sni_extension(&sni_data).is_err());
    }
}
