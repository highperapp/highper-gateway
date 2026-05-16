//! Crypto key extraction for kTLS
//!
//! Extracts session keys from rustls connections for kTLS configuration

use super::{CipherSuite, SessionKeys, TlsVersion};
use std::io;
use tracing::debug;

/// Extract session keys from a TLS connection
///
/// This is a placeholder interface. In a real implementation, this would:
/// 1. Access the rustls::ConnectionCommon internal state
/// 2. Extract the cipher suite being used
/// 3. Extract the session keys (traffic secrets)
/// 4. Extract the IV and salt
/// 5. Get the current sequence number
///
/// Note: rustls does not expose these internals by default, so this requires
/// either:
/// - Forking rustls and adding an extraction API
/// - Using unsafe code to access private fields
/// - Implementing a custom TLS provider that tracks keys
pub fn extract_session_keys_from_rustls(
    _connection: &dyn std::any::Any,
) -> io::Result<SessionKeys> {
    // This is a stub implementation
    // Real implementation would need rustls integration

    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Session key extraction from rustls not yet implemented",
    ))
}

/// Map rustls cipher suite to kTLS cipher suite
pub fn map_rustls_cipher_suite(suite_name: &str) -> Option<CipherSuite> {
    match suite_name {
        "TLS13_AES_128_GCM_SHA256" => Some(CipherSuite::Aes128Gcm),
        "TLS13_AES_256_GCM_SHA384" => Some(CipherSuite::Aes256Gcm),
        "TLS13_CHACHA20_POLY1305_SHA256" => Some(CipherSuite::Chacha20Poly1305),
        "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256" => Some(CipherSuite::Aes128Gcm),
        "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384" => Some(CipherSuite::Aes256Gcm),
        _ => {
            debug!("Unsupported cipher suite for kTLS: {}", suite_name);
            None
        }
    }
}

/// Determine TLS version from protocol version
pub fn map_protocol_version(version: u16) -> Option<TlsVersion> {
    match version {
        0x0303 => Some(TlsVersion::Tls12),
        0x0304 => Some(TlsVersion::Tls13),
        _ => None,
    }
}

/// Helper to create session keys manually (for testing)
pub fn create_test_session_keys(version: TlsVersion, cipher_suite: CipherSuite) -> SessionKeys {
    let key_size = cipher_suite.key_size();
    let iv_size = cipher_suite.iv_size();
    let salt_size = cipher_suite.salt_size();

    SessionKeys {
        version,
        cipher_suite,
        key: vec![0u8; key_size],
        iv: vec![0u8; iv_size],
        salt: vec![0u8; salt_size],
        seq_num: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_rustls_cipher_suite() {
        assert_eq!(
            map_rustls_cipher_suite("TLS13_AES_128_GCM_SHA256"),
            Some(CipherSuite::Aes128Gcm)
        );
        assert_eq!(
            map_rustls_cipher_suite("TLS13_AES_256_GCM_SHA384"),
            Some(CipherSuite::Aes256Gcm)
        );
        assert_eq!(
            map_rustls_cipher_suite("TLS13_CHACHA20_POLY1305_SHA256"),
            Some(CipherSuite::Chacha20Poly1305)
        );
        assert_eq!(map_rustls_cipher_suite("UNKNOWN_CIPHER"), None);
    }

    #[test]
    fn test_map_protocol_version() {
        assert_eq!(map_protocol_version(0x0303), Some(TlsVersion::Tls12));
        assert_eq!(map_protocol_version(0x0304), Some(TlsVersion::Tls13));
        assert_eq!(map_protocol_version(0x0301), None); // TLS 1.0
    }

    #[test]
    fn test_create_test_keys() {
        let keys = create_test_session_keys(TlsVersion::Tls13, CipherSuite::Aes256Gcm);

        assert_eq!(keys.version, TlsVersion::Tls13);
        assert_eq!(keys.cipher_suite, CipherSuite::Aes256Gcm);
        assert_eq!(keys.key.len(), 32);
        assert_eq!(keys.iv.len(), 8);
        assert_eq!(keys.salt.len(), 4);
    }
}
