//! Certificate storage implementation

use crate::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info};

/// Certificate with its private key
#[derive(Debug, Clone)]
pub struct Certificate {
    /// Domain name
    pub domain: String,
    /// Certificate chain in PEM format
    pub cert_pem: Vec<u8>,
    /// Private key in PEM format
    pub key_pem: Vec<u8>,
}

/// Certificate storage backend
pub trait CertificateStorage: Send + Sync {
    /// Store a certificate
    fn store(&self, cert: &Certificate) -> Result<()>;

    /// Retrieve a certificate by domain
    fn get(&self, domain: &str) -> Result<Option<Certificate>>;

    /// List all stored certificates
    fn list(&self) -> Result<Vec<String>>;

    /// Delete a certificate
    fn delete(&self, domain: &str) -> Result<()>;
}

/// File-based certificate storage
pub struct FileStorage {
    base_path: PathBuf,
    /// In-memory cache for faster lookups
    cache: Arc<RwLock<HashMap<String, Certificate>>>,
}

impl FileStorage {
    /// Create a new file storage
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();

        // Create storage directory if it doesn't exist
        if !base_path.exists() {
            info!(
                "Creating certificate storage directory: {}",
                base_path.display()
            );
            fs::create_dir_all(&base_path)?;
        }

        // Load existing certificates into cache
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let storage = Self { base_path, cache };

        // Pre-load certificates
        storage.load_all()?;

        Ok(storage)
    }

    /// Load all certificates from disk into cache
    fn load_all(&self) -> Result<()> {
        let entries = fs::read_dir(&self.base_path)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(domain) = path.file_name().and_then(|n| n.to_str()) {
                    match self.load_from_disk(domain) {
                        Ok(Some(cert)) => {
                            debug!("Loaded certificate for domain: {}", domain);
                            self.cache.write().insert(domain.to_string(), cert);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            error!("Failed to load certificate for {}: {}", domain, e);
                        }
                    }
                }
            }
        }

        info!(
            "Loaded {} certificates from storage",
            self.cache.read().len()
        );
        Ok(())
    }

    /// Load certificate from disk
    fn load_from_disk(&self, domain: &str) -> Result<Option<Certificate>> {
        let domain_path = self.base_path.join(domain);
        let cert_path = domain_path.join("cert.pem");
        let key_path = domain_path.join("key.pem");

        if !cert_path.exists() || !key_path.exists() {
            return Ok(None);
        }

        let cert_pem = fs::read(&cert_path)?;
        let key_pem = fs::read(&key_path)?;

        Ok(Some(Certificate {
            domain: domain.to_string(),
            cert_pem,
            key_pem,
        }))
    }

    /// Save certificate to disk
    fn save_to_disk(&self, cert: &Certificate) -> Result<()> {
        let domain_path = self.base_path.join(&cert.domain);

        // Create domain directory
        fs::create_dir_all(&domain_path)?;

        // Write certificate and key
        let cert_path = domain_path.join("cert.pem");
        let key_path = domain_path.join("key.pem");

        fs::write(&cert_path, &cert.cert_pem)?;
        fs::write(&key_path, &cert.key_pem)?;

        // Set restrictive permissions on private key (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&key_path)?.permissions();
            perms.set_mode(0o600); // Read/write for owner only
            fs::set_permissions(&key_path, perms)?;
        }

        debug!("Saved certificate for domain: {}", cert.domain);
        Ok(())
    }
}

impl CertificateStorage for FileStorage {
    fn store(&self, cert: &Certificate) -> Result<()> {
        // Save to disk
        self.save_to_disk(cert)?;

        // Update cache
        self.cache.write().insert(cert.domain.clone(), cert.clone());

        info!("Stored certificate for domain: {}", cert.domain);
        Ok(())
    }

    fn get(&self, domain: &str) -> Result<Option<Certificate>> {
        // Check cache first
        if let Some(cert) = self.cache.read().get(domain) {
            return Ok(Some(cert.clone()));
        }

        // Try loading from disk
        if let Some(cert) = self.load_from_disk(domain)? {
            // Update cache
            self.cache.write().insert(domain.to_string(), cert.clone());
            return Ok(Some(cert));
        }

        Ok(None)
    }

    fn list(&self) -> Result<Vec<String>> {
        Ok(self.cache.read().keys().cloned().collect())
    }

    fn delete(&self, domain: &str) -> Result<()> {
        // Remove from cache
        self.cache.write().remove(domain);

        // Remove from disk
        let domain_path = self.base_path.join(domain);
        if domain_path.exists() {
            fs::remove_dir_all(domain_path)?;
            info!("Deleted certificate for domain: {}", domain);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_file_storage_store_and_get() {
        let temp_path = temp_dir().join("highper-gateway-test-certs");
        let storage = FileStorage::new(&temp_path).unwrap();

        let cert = Certificate {
            domain: "example.com".to_string(),
            cert_pem: b"test cert".to_vec(),
            key_pem: b"test key".to_vec(),
        };

        // Store certificate
        storage.store(&cert).unwrap();

        // Retrieve certificate
        let retrieved = storage.get("example.com").unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.domain, "example.com");
        assert_eq!(retrieved.cert_pem, b"test cert");
        assert_eq!(retrieved.key_pem, b"test key");

        // Cleanup
        let _ = fs::remove_dir_all(temp_path);
    }
}
