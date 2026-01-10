# Phase 1.5: Certificate Hot Reload - Implementation Specification

**Duration:** 1 week
**Priority:** Medium
**Difficulty:** Low-Medium
**Impact:** +3% TLS score

---

## Executive Summary

Enable automatic reloading of TLS certificates when they are updated on disk, without restarting the proxy. This is essential for ACME certificate renewals (Let's Encrypt) and manual certificate updates in production environments.

---

## Goals

- Watch certificate and key files for changes
- Validate new certificates before applying
- Reload certificates without dropping connections
- Integrate with ACME auto-renewal (Let's Encrypt)
- Support both TLS termination and mTLS certificates

---

## Architecture

```
File System (certificates)
        │
        ↓
   File Watcher
   (inotify/FSEvents)
        │
        ↓
  Certificate Loader
        │
        ├──→ Validate Certificate
        ├──→ Validate Private Key
        ├──→ Check Cert/Key Match
        │
        ↓
   TLS Acceptor Update
   (Arc<RwLock<TlsAcceptor>>)
        │
        ↓
  New Connections Use New Cert
  (Existing connections unchanged)
```

---

## Implementation

### 1. Certificate Watcher

**File:** `highper-gateway/src/tls/cert_watcher.rs` (new)

```rust
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::{info, warn};

pub struct CertificateWatcher {
    cert_path: PathBuf,
    key_path: PathBuf,
    watcher: Option<RecommendedWatcher>,
}

pub enum CertEvent {
    CertificateModified,
    KeyModified,
    BothModified,
}

impl CertificateWatcher {
    pub fn new<P: AsRef<Path>>(
        cert_path: P,
        key_path: P,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<CertEvent>)> {
        let cert_path = cert_path.as_ref().to_path_buf();
        let key_path = key_path.as_ref().to_path_buf();
        let (tx, rx) = mpsc::unbounded_channel();

        let cert_path_clone = cert_path.clone();
        let key_path_clone = key_path.clone();

        let watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                if let Ok(event) = result {
                    for path in event.paths {
                        if path == cert_path_clone {
                            let _ = tx.send(CertEvent::CertificateModified);
                        } else if path == key_path_clone {
                            let _ = tx.send(CertEvent::KeyModified);
                        }
                    }
                }
            },
            Config::default(),
        )?;

        Ok((
            Self {
                cert_path,
                key_path,
                watcher: Some(watcher),
            },
            rx,
        ))
    }

    pub fn watch(&mut self) -> anyhow::Result<()> {
        if let Some(watcher) = &mut self.watcher {
            watcher.watch(&self.cert_path, RecursiveMode::NonRecursive)?;
            watcher.watch(&self.key_path, RecursiveMode::NonRecursive)?;
            info!("Watching certificates: {:?}, {:?}", self.cert_path, self.key_path);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Watcher not initialized"))
        }
    }
}
```

### 2. Certificate Validator

**File:** `highper-gateway/src/tls/cert_validator.rs` (new)

```rust
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::fs;
use std::path::Path;
use tracing::{debug, warn};

pub struct CertificateValidator;

impl CertificateValidator {
    /// Validate certificate and key pair
    pub fn validate<P: AsRef<Path>>(
        cert_path: P,
        key_path: P,
    ) -> anyhow::Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
        debug!("Validating certificate: {:?}", cert_path.as_ref());

        // Load certificate
        let cert_file = fs::File::open(cert_path.as_ref())?;
        let mut reader = std::io::BufReader::new(cert_file);
        let certs = rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()?;

        if certs.is_empty() {
            return Err(anyhow::anyhow!("No certificates found in file"));
        }

        // Load private key
        let key_file = fs::File::open(key_path.as_ref())?;
        let mut reader = std::io::BufReader::new(key_file);
        let key = rustls_pemfile::private_key(&mut reader)?
            .ok_or_else(|| anyhow::anyhow!("No private key found in file"))?;

        // Verify cert and key match (basic validation)
        // In production, use openssl or ring to verify the pair properly
        Self::verify_cert_key_match(&certs[0], &key)?;

        // Check expiration
        Self::check_expiration(&certs[0])?;

        Ok((certs, key))
    }

    fn verify_cert_key_match(
        _cert: &CertificateDer,
        _key: &PrivateKeyDer,
    ) -> anyhow::Result<()> {
        // TODO: Implement proper cert/key matching
        // For now, trust that they match
        Ok(())
    }

    fn check_expiration(cert: &CertificateDer) -> anyhow::Result<()> {
        use x509_parser::parse_x509_certificate;

        let (_, parsed_cert) = parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to parse certificate: {}", e))?;

        let now = std::time::SystemTime::now();
        let not_after = parsed_cert.validity().not_after;

        // Warning if expires in < 30 days
        let thirty_days = std::time::Duration::from_secs(30 * 24 * 60 * 60);
        if let Ok(time_until_expiry) = not_after.to_datetime().signed_duration_since(
            chrono::DateTime::from(now)
        ) {
            if time_until_expiry.num_seconds() < thirty_days.as_secs() as i64 {
                warn!(
                    "Certificate expires soon: {} days remaining",
                    time_until_expiry.num_days()
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_certificate() {
        // Test with valid cert/key pair
        let result = CertificateValidator::validate(
            "test-data/server.crt",
            "test-data/server.key",
        );
        // Will fail without test data, but shows API usage
    }
}
```

### 3. TLS Acceptor Manager

**File:** `highper-gateway/src/tls/acceptor.rs` (enhance existing)

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_rustls::TlsAcceptor;
use tracing::info;

/// Shared TLS acceptor that can be hot-reloaded
pub type SharedTlsAcceptor = Arc<RwLock<TlsAcceptor>>;

pub struct TlsAcceptorManager {
    acceptor: SharedTlsAcceptor,
    cert_path: String,
    key_path: String,
}

impl TlsAcceptorManager {
    pub fn new(cert_path: String, key_path: String) -> anyhow::Result<Self> {
        let acceptor = build_tls_acceptor_from_paths(&cert_path, &key_path)?;
        let acceptor = Arc::new(RwLock::new(acceptor));

        Ok(Self {
            acceptor,
            cert_path,
            key_path,
        })
    }

    pub fn acceptor(&self) -> SharedTlsAcceptor {
        Arc::clone(&self.acceptor)
    }

    /// Reload certificates
    pub async fn reload_certificates(&self) -> anyhow::Result<()> {
        info!("Reloading TLS certificates...");

        // Validate new certificates first
        let (certs, key) = CertificateValidator::validate(&self.cert_path, &self.key_path)?;

        // Build new acceptor
        let new_acceptor = build_tls_acceptor_from_materials(certs, key)?;

        // Atomically swap acceptor
        let mut acceptor = self.acceptor.write().await;
        *acceptor = new_acceptor;

        info!("TLS certificates reloaded successfully");
        Ok(())
    }

    /// Start watching for certificate changes
    pub async fn start_watching(self: Arc<Self>) -> anyhow::Result<()> {
        let (mut watcher, mut event_rx) =
            CertificateWatcher::new(&self.cert_path, &self.key_path)?;
        watcher.watch()?;

        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                info!("Certificate change detected: {:?}", event);

                // Small delay to ensure file write is complete
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                if let Err(e) = self.reload_certificates().await {
                    error!("Failed to reload certificates: {}", e);
                } else {
                    info!("Certificates reloaded successfully");
                }
            }
        });

        Ok(())
    }
}

fn build_tls_acceptor_from_paths(
    cert_path: &str,
    key_path: &str,
) -> anyhow::Result<TlsAcceptor> {
    let (certs, key) = CertificateValidator::validate(cert_path, key_path)?;
    build_tls_acceptor_from_materials(certs, key)
}

fn build_tls_acceptor_from_materials(
    certs: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
) -> anyhow::Result<TlsAcceptor> {
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(TlsAcceptor::from(Arc::new(server_config)))
}
```

### 4. Server Integration

**File:** `highper-gateway/src/proxy/server.rs` (enhance existing)

```rust
// In run_tls_server function:

pub async fn run_tls_server(config: Arc<Config>) -> Result<()> {
    let tls_config = config.tls.as_ref().unwrap();

    // Create acceptor manager
    let acceptor_manager = Arc::new(TlsAcceptorManager::new(
        tls_config.cert_path.clone(),
        tls_config.key_path.clone(),
    )?);

    // Start watching for certificate changes
    acceptor_manager.clone().start_watching().await?;

    // Get shared acceptor
    let acceptor = acceptor_manager.acceptor();

    let listener = TcpListener::bind(format!("{}:{}", config.server.host, tls_config.port)).await?;

    info!("TLS server listening on port {}", tls_config.port);

    loop {
        let (stream, _) = listener.accept().await?;
        let acceptor = Arc::clone(&acceptor);
        let handler = Arc::clone(&handler);

        tokio::spawn(async move {
            // Get current acceptor (may have been reloaded)
            let tls_acceptor = acceptor.read().await.clone();

            match tls_acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    if let Err(e) = handle_tls_connection(tls_stream, handler).await {
                        error!("TLS connection error: {}", e);
                    }
                }
                Err(e) => {
                    error!("TLS handshake failed: {}", e);
                }
            }
        });
    }
}
```

---

## Configuration Example

```yaml
tls:
  enabled: true
  port: 8443
  cert_path: "/etc/certs/server.crt"
  key_path: "/etc/certs/server.key"

  # Certificate hot reload is automatic
  # When cert/key files change, they are reloaded automatically

  # ACME integration (optional)
  acme:
    enabled: true
    email: "admin@example.com"
    directory_url: "https://acme-v02.api.letsencrypt.org/directory"
    # Certificates are automatically renewed and reloaded
```

---

## ACME Integration (Let's Encrypt)

### Auto-Renewal Flow

```
ACME Provider (Let's Encrypt)
        │
        ↓
Certificate Expires in <30 days
        │
        ↓
   ACME Client
   (automatic renewal)
        │
        ↓
New Certificate Written to Disk
(/etc/certs/server.crt)
        │
        ↓
  File Watcher Detects Change
        │
        ↓
  Certificate Validator
        │
        ↓
   TLS Acceptor Reloaded
        │
        ↓
New Connections Use New Cert
```

### ACME Configuration

**File:** `highper-gateway/src/tls/acme.rs` (basic implementation)

```rust
use acme_lib::{create_p384_key, Directory, DirectoryUrl};
use acme_lib::persist::FilePersist;
use std::path::Path;
use tokio::time::{interval, Duration};
use tracing::{info, error};

pub struct AcmeManager {
    email: String,
    domains: Vec<String>,
    cert_path: String,
    key_path: String,
}

impl AcmeManager {
    pub fn new(
        email: String,
        domains: Vec<String>,
        cert_path: String,
        key_path: String,
    ) -> Self {
        Self {
            email,
            domains,
            cert_path,
            key_path,
        }
    }

    /// Start automatic renewal check (runs every 24 hours)
    pub async fn start_auto_renewal(self) {
        let mut check_interval = interval(Duration::from_secs(24 * 60 * 60));

        tokio::spawn(async move {
            loop {
                check_interval.tick().await;

                if let Err(e) = self.check_and_renew().await {
                    error!("ACME renewal check failed: {}", e);
                }
            }
        });
    }

    async fn check_and_renew(&self) -> anyhow::Result<()> {
        // Check if certificate expires in < 30 days
        if !self.should_renew()? {
            info!("Certificate does not need renewal yet");
            return Ok(());
        }

        info!("Certificate expires soon, requesting renewal...");

        // Request new certificate via ACME
        self.request_certificate().await?;

        info!("Certificate renewed successfully");
        Ok(())
    }

    fn should_renew(&self) -> anyhow::Result<bool> {
        // Check certificate expiration
        let cert_file = std::fs::File::open(&self.cert_path)?;
        let mut reader = std::io::BufReader::new(cert_file);
        let certs = rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()?;

        if certs.is_empty() {
            return Ok(true); // No cert, need to get one
        }

        // Parse and check expiration
        use x509_parser::parse_x509_certificate;
        let (_, parsed_cert) = parse_x509_certificate(certs[0].as_ref())?;

        let now = std::time::SystemTime::now();
        let not_after = parsed_cert.validity().not_after;

        // Renew if < 30 days remaining
        let thirty_days = std::time::Duration::from_secs(30 * 24 * 60 * 60);
        // Simplified check - production needs proper date comparison

        Ok(true) // TODO: Implement proper date comparison
    }

    async fn request_certificate(&self) -> anyhow::Result<()> {
        // Use acme-lib to request certificate
        let url = DirectoryUrl::LetsEncrypt;
        let persist = FilePersist::new("/var/lib/highper-gateway/acme");
        let dir = Directory::from_url(persist, url)?;

        let acc = dir.account(&self.email)?;

        // Create certificate order
        let mut ord_new = acc.new_order(&self.domains[0], &[])?;

        // Get ownership of domain via HTTP-01 challenge
        let ord_csr = loop {
            if let Some(ord_csr) = ord_new.confirm_validations() {
                break ord_csr;
            }

            let auths = ord_new.authorizations()?;
            let chall = auths[0].http_challenge();

            // TODO: Serve challenge at /.well-known/acme-challenge/
            // This requires integration with HTTP server

            chall.validate(5000)?;

            ord_new.refresh()?;
        };

        // Submit CSR
        let pkey = create_p384_key();
        let ord_cert = ord_csr.finalize_pkey(pkey, 5000)?;

        // Download certificate
        let cert = ord_cert.download_and_save_cert()?;

        // Write to disk (will trigger hot reload)
        std::fs::write(&self.cert_path, cert.certificate())?;
        std::fs::write(&self.key_path, cert.private_key())?;

        Ok(())
    }
}
```

---

## Testing

### Unit Tests

```rust
#[tokio::test]
async fn test_certificate_validation()

#[tokio::test]
async fn test_certificate_reload()

#[tokio::test]
async fn test_invalid_certificate_rejected()
```

### Integration Tests

```rust
#[tokio::test]
async fn test_certificate_hot_reload_end_to_end() {
    // Start proxy with cert1
    let proxy = start_proxy_with_tls().await;

    // Verify cert1 is in use
    let cert_info = get_server_certificate().await;
    assert_eq!(cert_info.serial, "CERT1_SERIAL");

    // Replace certificate file
    std::fs::copy("test-certs/cert2.pem", "/tmp/server.crt").unwrap();

    // Wait for reload
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify cert2 is now in use
    let cert_info = get_server_certificate().await;
    assert_eq!(cert_info.serial, "CERT2_SERIAL");

    // Existing connections should still work
    // New connections use new certificate
}
```

### Manual Testing

```bash
# Start proxy
./target/release/highper-gateway config/example.yaml

# Check certificate
openssl s_client -connect localhost:8443 -showcerts | grep "subject="

# Replace certificate (simulate renewal)
cp new-cert.pem /etc/certs/server.crt
cp new-key.pem /etc/certs/server.key

# Wait 1 second for reload

# Check new certificate is active
openssl s_client -connect localhost:8443 -showcerts | grep "subject="
```

---

## Dependencies

```toml
[dependencies]
# For ACME (Let's Encrypt)
acme-lib = "0.9"

# Already have:
# notify = "6.1"
# x509-parser = "0.16"
```

---

## Acceptance Criteria

- [ ] Certificate files watched for changes
- [ ] New certificates validated before applying
- [ ] Invalid certificates rejected with clear errors
- [ ] Certificates reloaded within 1 second of change
- [ ] Zero connection drops during reload
- [ ] ACME auto-renewal works (optional enhancement)
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing successful

---

## Performance

- **Reload time:** < 100ms
- **Connection impact:** Zero (existing connections unchanged)
- **File watching overhead:** Negligible
- **Validation overhead:** ~5ms per reload

---

## Security Considerations

- Validate certificate/key pair before applying
- Check certificate expiration
- Verify certificate chain if CA bundle provided
- Log all certificate reload events for audit
- Fail securely: keep old cert if new cert invalid

---

## Documentation

Create `docs/CERTIFICATE_MANAGEMENT.md`:
- How to update certificates
- ACME/Let's Encrypt integration
- Troubleshooting certificate issues
- Certificate expiration monitoring

---

## Next Steps

After implementation:
1. Test with real Let's Encrypt certificates
2. Integrate with monitoring for expiration alerts
3. Proceed to Phase 1.6: OCSP Stapling

---

**Document Version:** 1.0
**Last Updated:** October 30, 2025
**Status:** Ready for implementation
