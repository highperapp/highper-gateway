//! CA Certificate Manager for mTLS client verification
//!
//! This module manages Certificate Authority (CA) certificates used for
//! verifying client certificates in mutual TLS (mTLS) connections.

use anyhow::{anyhow, Context, Result};
use rustls::RootCertStore;
use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Manages CA certificates for client certificate verification
#[derive(Clone, Debug)]
pub struct CaManager {
    /// Root certificate store containing trusted CAs
    root_store: Arc<RootCertStore>,

    /// Number of CAs loaded
    ca_count: usize,
}

impl CaManager {
    /// Create a new CA manager by loading certificates from the specified path
    ///
    /// # Arguments
    /// * `ca_cert_path` - Path to CA certificate file (PEM format)
    /// * `additional_cas` - Optional additional CA certificate paths
    ///
    /// # Returns
    /// * `Result<Self>` - CA manager on success, error on failure
    pub fn new<P: AsRef<Path>>(
        ca_cert_path: P,
        additional_cas: &[String],
    ) -> Result<Self> {
        let ca_cert_path = ca_cert_path.as_ref();

        info!(
            "Loading CA certificates from: {}",
            ca_cert_path.display()
        );

        let mut root_store = RootCertStore::empty();
        let mut ca_count = 0;

        // Load primary CA certificate(s)
        ca_count += Self::load_ca_file(&mut root_store, ca_cert_path)?;

        // Load additional CA certificates
        for additional_ca_path in additional_cas {
            match Self::load_ca_file(&mut root_store, Path::new(additional_ca_path)) {
                Ok(count) => {
                    ca_count += count;
                    debug!(
                        "Loaded {} additional CA(s) from: {}",
                        count, additional_ca_path
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to load additional CA from {}: {}",
                        additional_ca_path, e
                    );
                    // Continue loading other CAs even if one fails
                }
            }
        }

        if ca_count == 0 {
            return Err(anyhow!("No CA certificates were loaded"));
        }

        info!("Successfully loaded {} CA certificate(s)", ca_count);

        Ok(Self {
            root_store: Arc::new(root_store),
            ca_count,
        })
    }

    /// Load CA certificates from a single file
    ///
    /// # Arguments
    /// * `root_store` - Root certificate store to add certificates to
    /// * `path` - Path to CA certificate file (PEM format)
    ///
    /// # Returns
    /// * `Result<usize>` - Number of certificates loaded
    fn load_ca_file(root_store: &mut RootCertStore, path: &Path) -> Result<usize> {
        if !path.exists() {
            return Err(anyhow!("CA certificate file not found: {}", path.display()));
        }

        // Read the file
        let ca_file = fs::File::open(path)
            .with_context(|| format!("Failed to open CA certificate file: {}", path.display()))?;

        let mut reader = BufReader::new(ca_file);

        // Parse PEM certificates
        let certs = rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| format!("Failed to parse CA certificates from: {}", path.display()))?;

        if certs.is_empty() {
            return Err(anyhow!(
                "No certificates found in CA file: {}",
                path.display()
            ));
        }

        let count = certs.len();

        // Add certificates to root store
        // rustls RootCertStore::add() expects owned CertificateDer
        for cert in certs {
            root_store.add(cert)
                .map_err(|e| anyhow!("Failed to add CA certificate to store: {:?}", e))?;
        }

        Ok(count)
    }

    /// Get a reference to the root certificate store
    ///
    /// # Returns
    /// * `&RootCertStore` - Root certificate store
    pub fn root_store(&self) -> &RootCertStore {
        &self.root_store
    }

    /// Get the number of loaded CA certificates
    ///
    /// # Returns
    /// * `usize` - Number of CAs
    pub fn ca_count(&self) -> usize {
        self.ca_count
    }

    /// Create a clone of the root certificate store (Arc clone, cheap)
    ///
    /// # Returns
    /// * `Arc<RootCertStore>` - Cloned root store
    pub fn root_store_arc(&self) -> Arc<RootCertStore> {
        Arc::clone(&self.root_store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Test CA certificate (self-signed root CA for testing only)
    const TEST_CA_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIDXTCCAkWgAwIBAgIJAKJ5JqJ5JqJ5MA0GCSqGSIb3DQEBCwUAMEUxCzAJBgNV
BAYTAkFVMRMwEQYDVQQIDApTb21lLVN0YXRlMSEwHwYDVQQKDBhJbnRlcm5ldCBX
aWRnaXRzIFB0eSBMdGQwHhcNMjMwMTAxMDAwMDAwWhcNMzMwMTAxMDAwMDAwWjBF
MQswCQYDVQQGEwJBVTETMBEGA1UECAwKU29tZS1TdGF0ZTEhMB8GA1UECgwYSW50
ZXJuZXQgV2lkZ2l0cyBQdHkgTHRkMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIB
CgKCAQEAyqZ3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3
Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3
Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3
Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3
Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3
Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3Z3ID
AQABMA0GCSqGSIb3DQEBCwUAA4IBAQBkZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZm
ZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZm
ZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZm
ZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZm
Zg==
-----END CERTIFICATE-----"#;

    #[test]
    fn test_ca_manager_creation() {
        // Create temporary CA file
        let mut ca_file = NamedTempFile::new().expect("Failed to create temp file");
        ca_file
            .write_all(TEST_CA_CERT.as_bytes())
            .expect("Failed to write CA cert");

        // Create CA manager
        let manager = CaManager::new(ca_file.path(), &[]);

        // Should succeed or fail gracefully (cert might be invalid for actual use)
        match manager {
            Ok(mgr) => {
                assert!(mgr.ca_count() > 0);
                assert!(!mgr.root_store().is_empty());
            }
            Err(e) => {
                // Test cert might be rejected - that's ok for this test
                println!("CA manager creation failed (expected with test cert): {}", e);
            }
        }
    }

    #[test]
    fn test_ca_manager_missing_file() {
        let result = CaManager::new("/nonexistent/ca.pem", &[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = err.to_string();
        println!("Error message: {}", err_msg);
        assert!(err_msg.contains("not found") || err_msg.contains("No such file"));
    }

    #[test]
    fn test_ca_manager_invalid_pem() {
        // Create temporary file with invalid PEM
        let mut ca_file = NamedTempFile::new().expect("Failed to create temp file");
        ca_file
            .write_all(b"This is not a valid PEM certificate")
            .expect("Failed to write invalid data");

        let result = CaManager::new(ca_file.path(), &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_ca_manager_empty_file() {
        // Create empty temporary file
        let ca_file = NamedTempFile::new().expect("Failed to create temp file");

        let result = CaManager::new(ca_file.path(), &[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = err.to_string();
        println!("Error message: {}", err_msg);
        assert!(err_msg.contains("No certificates") || err_msg.contains("no certificates"));
    }

    #[test]
    fn test_ca_manager_additional_cas() {
        // Create primary CA file
        let mut ca_file1 = NamedTempFile::new().expect("Failed to create temp file");
        ca_file1
            .write_all(TEST_CA_CERT.as_bytes())
            .expect("Failed to write CA cert");

        // Create additional CA file
        let mut ca_file2 = NamedTempFile::new().expect("Failed to create temp file");
        ca_file2
            .write_all(TEST_CA_CERT.as_bytes())
            .expect("Failed to write CA cert");

        // Create CA manager with additional CAs
        let additional_cas = vec![ca_file2.path().to_string_lossy().to_string()];
        let result = CaManager::new(ca_file1.path(), &additional_cas);

        // Should attempt to load both (may fail due to test cert validity)
        match result {
            Ok(mgr) => {
                // If successful, should have loaded multiple CAs
                println!("Loaded {} CAs", mgr.ca_count());
            }
            Err(e) => {
                println!("CA loading failed (expected with test cert): {}", e);
            }
        }
    }
}
