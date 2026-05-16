//! Configuration for kernel TLS

use serde::{Deserialize, Serialize};

/// Kernel TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KTlsConfig {
    /// Enable kernel TLS
    #[serde(default)]
    pub enabled: bool,

    /// Fallback to userspace TLS if kTLS is not available
    #[serde(default = "default_true")]
    pub fallback_to_userspace: bool,

    /// Supported TLS versions for kTLS
    #[serde(default = "default_tls_versions")]
    pub tls_versions: Vec<TlsVersion>,

    /// Enable zero-copy sendfile for static content
    #[serde(default = "default_true")]
    pub enable_sendfile: bool,

    /// Cipher suites supported by kTLS
    #[serde(default = "default_cipher_suites")]
    pub cipher_suites: Vec<CipherSuite>,

    /// Enable kTLS for receive (RX) operations
    #[serde(default)]
    pub enable_rx: bool,

    /// Enable kTLS for transmit (TX) operations
    #[serde(default = "default_true")]
    pub enable_tx: bool,
}

impl Default for KTlsConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default (opt-in)
            fallback_to_userspace: true,
            tls_versions: default_tls_versions(),
            enable_sendfile: true,
            cipher_suites: default_cipher_suites(),
            enable_rx: false, // RX support is more experimental
            enable_tx: true,  // TX is well-tested and provides most benefits
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_tls_versions() -> Vec<TlsVersion> {
    vec![TlsVersion::Tls13, TlsVersion::Tls12]
}

fn default_cipher_suites() -> Vec<CipherSuite> {
    vec![
        CipherSuite::Aes128Gcm,
        CipherSuite::Aes256Gcm,
        CipherSuite::Chacha20Poly1305,
    ]
}

/// TLS version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TlsVersion {
    /// TLS 1.2 (requires Linux 4.13+)
    #[serde(rename = "1.2")]
    Tls12,

    /// TLS 1.3 (requires Linux 4.17+)
    #[serde(rename = "1.3")]
    Tls13,
}

impl TlsVersion {
    /// Get minimum kernel version required
    pub fn min_kernel_version(&self) -> (u32, u32, u32) {
        match self {
            TlsVersion::Tls12 => (4, 13, 0),
            TlsVersion::Tls13 => (4, 17, 0),
        }
    }

    /// Get protocol version number for setsockopt
    pub fn protocol_version(&self) -> u16 {
        match self {
            TlsVersion::Tls12 => 0x0303, // TLS 1.2
            TlsVersion::Tls13 => 0x0304, // TLS 1.3
        }
    }
}

/// Cipher suite supported by kTLS
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CipherSuite {
    /// AES-128-GCM
    #[serde(rename = "AES_128_GCM")]
    Aes128Gcm,

    /// AES-256-GCM
    #[serde(rename = "AES_256_GCM")]
    Aes256Gcm,

    /// ChaCha20-Poly1305
    #[serde(rename = "CHACHA20_POLY1305")]
    Chacha20Poly1305,
}

impl CipherSuite {
    /// Get cipher suite ID for setsockopt
    #[cfg(target_os = "linux")]
    pub fn cipher_type(&self) -> u16 {
        match self {
            CipherSuite::Aes128Gcm => 51,        // TLS_CIPHER_AES_GCM_128
            CipherSuite::Aes256Gcm => 52,        // TLS_CIPHER_AES_GCM_256
            CipherSuite::Chacha20Poly1305 => 54, // TLS_CIPHER_CHACHA20_POLY1305
        }
    }

    /// Get key size in bytes
    pub fn key_size(&self) -> usize {
        match self {
            CipherSuite::Aes128Gcm => 16,        // 128 bits
            CipherSuite::Aes256Gcm => 32,        // 256 bits
            CipherSuite::Chacha20Poly1305 => 32, // 256 bits
        }
    }

    /// Get IV (Initialization Vector) size in bytes
    pub fn iv_size(&self) -> usize {
        match self {
            CipherSuite::Aes128Gcm => 8, // TLS 1.2 explicit nonce
            CipherSuite::Aes256Gcm => 8,
            CipherSuite::Chacha20Poly1305 => 12, // Full nonce
        }
    }

    /// Get salt size in bytes (TLS 1.2 implicit nonce)
    pub fn salt_size(&self) -> usize {
        4 // All cipher suites use 4-byte salt
    }

    /// Get record sequence number size
    pub fn rec_seq_size(&self) -> usize {
        8 // All TLS versions use 64-bit sequence numbers
    }
}

/// kTLS statistics
#[derive(Debug, Clone, Default)]
pub struct KTlsStats {
    /// Number of connections using kTLS
    pub active_connections: u64,

    /// Number of successful kTLS enablements
    pub successful_enables: u64,

    /// Number of kTLS enable failures
    pub failed_enables: u64,

    /// Number of fallbacks to userspace TLS
    pub fallbacks: u64,

    /// Total bytes transmitted via kTLS
    pub tx_bytes: u64,

    /// Total bytes received via kTLS
    pub rx_bytes: u64,

    /// Number of sendfile operations
    pub sendfile_ops: u64,

    /// Bytes transferred via sendfile
    pub sendfile_bytes: u64,
}

impl KTlsStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment successful enable counter
    pub fn record_enable_success(&mut self) {
        self.successful_enables += 1;
        self.active_connections += 1;
    }

    /// Increment enable failure counter
    pub fn record_enable_failure(&mut self) {
        self.failed_enables += 1;
        self.fallbacks += 1;
    }

    /// Record connection closed
    pub fn record_connection_closed(&mut self) {
        if self.active_connections > 0 {
            self.active_connections -= 1;
        }
    }

    /// Record transmitted bytes
    pub fn record_tx_bytes(&mut self, bytes: u64) {
        self.tx_bytes += bytes;
    }

    /// Record received bytes
    pub fn record_rx_bytes(&mut self, bytes: u64) {
        self.rx_bytes += bytes;
    }

    /// Record sendfile operation
    pub fn record_sendfile(&mut self, bytes: u64) {
        self.sendfile_ops += 1;
        self.sendfile_bytes += bytes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = KTlsConfig::default();
        assert!(!config.enabled); // Disabled by default
        assert!(config.fallback_to_userspace);
        assert!(config.enable_tx);
        assert!(!config.enable_rx);
    }

    #[test]
    fn test_tls_version_requirements() {
        assert_eq!(TlsVersion::Tls12.min_kernel_version(), (4, 13, 0));
        assert_eq!(TlsVersion::Tls13.min_kernel_version(), (4, 17, 0));
    }

    #[test]
    fn test_cipher_suite_sizes() {
        assert_eq!(CipherSuite::Aes128Gcm.key_size(), 16);
        assert_eq!(CipherSuite::Aes256Gcm.key_size(), 32);
        assert_eq!(CipherSuite::Chacha20Poly1305.key_size(), 32);

        assert_eq!(CipherSuite::Aes128Gcm.rec_seq_size(), 8);
    }

    #[test]
    fn test_stats_tracking() {
        let mut stats = KTlsStats::new();

        stats.record_enable_success();
        assert_eq!(stats.successful_enables, 1);
        assert_eq!(stats.active_connections, 1);

        stats.record_tx_bytes(1024);
        assert_eq!(stats.tx_bytes, 1024);

        stats.record_sendfile(4096);
        assert_eq!(stats.sendfile_ops, 1);
        assert_eq!(stats.sendfile_bytes, 4096);

        stats.record_connection_closed();
        assert_eq!(stats.active_connections, 0);
    }

    #[test]
    fn test_config_serialization() {
        let config = KTlsConfig {
            enabled: true,
            fallback_to_userspace: true,
            tls_versions: vec![TlsVersion::Tls13],
            enable_sendfile: true,
            cipher_suites: vec![CipherSuite::Aes256Gcm],
            enable_rx: false,
            enable_tx: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: KTlsConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.enabled, deserialized.enabled);
        assert_eq!(config.tls_versions.len(), deserialized.tls_versions.len());
    }
}
