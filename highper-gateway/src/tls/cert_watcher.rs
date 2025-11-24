//! Certificate file watcher for hot reload
//!
//! Watches certificate and key files for changes and notifies when they are modified.

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// Events emitted when certificate files change
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertEvent {
    /// Certificate file was modified
    CertificateModified,
    /// Private key file was modified
    KeyModified,
    /// Both certificate and key were modified
    BothModified,
}

/// Watches certificate and key files for changes
pub struct CertificateWatcher {
    cert_path: PathBuf,
    key_path: PathBuf,
    _watcher: RecommendedWatcher,
}

impl CertificateWatcher {
    /// Create a new certificate watcher
    ///
    /// # Arguments
    /// * `cert_path` - Path to certificate file
    /// * `key_path` - Path to private key file
    ///
    /// # Returns
    /// * `Result<(Self, UnboundedReceiver<CertEvent>)>` - Watcher and event receiver
    pub fn new<P: AsRef<Path>>(
        cert_path: P,
        key_path: P,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<CertEvent>)> {
        let cert_path = cert_path.as_ref().to_path_buf();
        let key_path = key_path.as_ref().to_path_buf();

        info!(
            "Creating certificate watcher for cert={:?}, key={:?}",
            cert_path, key_path
        );

        let (tx, rx) = mpsc::unbounded_channel();

        // Clone paths for the closure
        let cert_path_clone = cert_path.clone();
        let key_path_clone = key_path.clone();

        // Create file system watcher
        let watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                match result {
                    Ok(event) => {
                        // Only process write/modify events
                        if matches!(
                            event.kind,
                            EventKind::Modify(_) | EventKind::Create(_)
                        ) {
                            let mut cert_modified = false;
                            let mut key_modified = false;

                            for path in event.paths {
                                debug!("File change detected: {:?}", path);

                                if path == cert_path_clone {
                                    cert_modified = true;
                                } else if path == key_path_clone {
                                    key_modified = true;
                                }
                            }

                            // Send appropriate event
                            let event = match (cert_modified, key_modified) {
                                (true, true) => Some(CertEvent::BothModified),
                                (true, false) => Some(CertEvent::CertificateModified),
                                (false, true) => Some(CertEvent::KeyModified),
                                (false, false) => None,
                            };

                            if let Some(event) = event {
                                if let Err(e) = tx.send(event) {
                                    error!("Failed to send certificate event: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("File watcher error: {}", e);
                    }
                }
            },
            Config::default(),
        )?;

        Ok((
            Self {
                cert_path,
                key_path,
                _watcher: watcher,
            },
            rx,
        ))
    }

    /// Start watching certificate files
    ///
    /// This must be called after creating the watcher to begin monitoring files.
    pub fn watch(&mut self) -> anyhow::Result<()> {
        info!("Starting to watch certificate files");

        // Watch both certificate and key files
        // Use parent directory watching for atomic operations
        if let Some(cert_parent) = self.cert_path.parent() {
            self._watcher
                .watch(cert_parent, RecursiveMode::NonRecursive)?;
            debug!("Watching certificate directory: {:?}", cert_parent);
        } else {
            self._watcher
                .watch(&self.cert_path, RecursiveMode::NonRecursive)?;
        }

        if let Some(key_parent) = self.key_path.parent() {
            // Only watch if different from cert parent
            if self.cert_path.parent() != Some(key_parent) {
                self._watcher
                    .watch(key_parent, RecursiveMode::NonRecursive)?;
                debug!("Watching key directory: {:?}", key_parent);
            }
        } else {
            self._watcher
                .watch(&self.key_path, RecursiveMode::NonRecursive)?;
        }

        info!(
            "Watching certificates: {:?}, {:?}",
            self.cert_path, self.key_path
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_certificate_watcher_creation() {
        let temp_dir = std::env::temp_dir();
        let cert_path = temp_dir.join("test_cert.pem");
        let key_path = temp_dir.join("test_key.pem");

        // Create test files
        fs::write(&cert_path, "test cert").unwrap();
        fs::write(&key_path, "test key").unwrap();

        let result = CertificateWatcher::new(&cert_path, &key_path);
        assert!(result.is_ok());

        // Cleanup
        let _ = fs::remove_file(&cert_path);
        let _ = fs::remove_file(&key_path);
    }

    #[tokio::test]
    async fn test_certificate_modification_detected() {
        let temp_dir = std::env::temp_dir();
        let cert_path = temp_dir.join("test_cert_modify.pem");
        let key_path = temp_dir.join("test_key_modify.pem");

        // Create test files
        fs::write(&cert_path, "test cert").unwrap();
        fs::write(&key_path, "test key").unwrap();

        let (mut watcher, mut rx) = CertificateWatcher::new(&cert_path, &key_path).unwrap();
        watcher.watch().unwrap();

        // Give watcher time to initialize
        sleep(Duration::from_millis(100)).await;

        // Modify certificate file
        fs::write(&cert_path, "new cert content").unwrap();

        // Wait for event
        tokio::select! {
            event = rx.recv() => {
                assert!(event.is_some());
                assert_eq!(event.unwrap(), CertEvent::CertificateModified);
            }
            _ = sleep(Duration::from_secs(2)) => {
                panic!("Timeout waiting for certificate modification event");
            }
        }

        // Cleanup
        let _ = fs::remove_file(&cert_path);
        let _ = fs::remove_file(&key_path);
    }

    #[tokio::test]
    async fn test_key_modification_detected() {
        let temp_dir = std::env::temp_dir();
        let cert_path = temp_dir.join("test_cert_key_modify.pem");
        let key_path = temp_dir.join("test_key_key_modify.pem");

        // Create test files
        fs::write(&cert_path, "test cert").unwrap();
        fs::write(&key_path, "test key").unwrap();

        let (mut watcher, mut rx) = CertificateWatcher::new(&cert_path, &key_path).unwrap();
        watcher.watch().unwrap();

        // Give watcher time to initialize
        sleep(Duration::from_millis(100)).await;

        // Modify key file
        fs::write(&key_path, "new key content").unwrap();

        // Wait for event
        tokio::select! {
            event = rx.recv() => {
                assert!(event.is_some());
                assert_eq!(event.unwrap(), CertEvent::KeyModified);
            }
            _ = sleep(Duration::from_secs(2)) => {
                panic!("Timeout waiting for key modification event");
            }
        }

        // Cleanup
        let _ = fs::remove_file(&cert_path);
        let _ = fs::remove_file(&key_path);
    }
}
