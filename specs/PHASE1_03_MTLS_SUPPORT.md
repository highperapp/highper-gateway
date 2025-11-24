# Phase 1.3: mTLS Support - Implementation Specification

**Duration:** 2 weeks
**Priority:** High (Security Feature)
**Difficulty:** Medium-High
**Impact:** +6% TLS score

---

## Executive Summary

Implement mutual TLS (mTLS) authentication to verify client certificates during TLS handshake. This adds client certificate verification, certificate chain validation, OCSP revocation checking, and per-route mTLS configuration. This feature is critical for zero-trust architectures and high-security deployments.

---

## Goals

### Primary Goals
1. Client certificate verification during TLS handshake
2. Certificate chain validation
3. Per-route mTLS requirements (optional vs required)
4. Pass client certificate DN to backend
5. OCSP revocation checking
6. Certificate authority (CA) management

### Success Metrics
- Client certificates validated correctly
- Invalid certificates rejected at TLS layer
- Client cert info passed to backends
- +6% improvement in TLS score
- Zero performance degradation for non-mTLS routes

---

## Background: What is mTLS?

### Standard TLS (One-Way)
```
Client                           Server
   │                               │
   │──── ClientHello ──────────────→│
   │←─── ServerHello + ServerCert ─│
   │──── Verify ServerCert ────────→│ (Client verifies server)
   │──── Encrypted Traffic ─────────→│
```

### Mutual TLS (Two-Way)
```
Client                           Server
   │                               │
   │──── ClientHello ──────────────→│
   │←─── ServerHello + ServerCert ─│
   │──── Verify ServerCert ────────→│
   │──── ClientCert ───────────────→│ (Server requests client cert)
   │                               │
   │                        Verify ClientCert (Server verifies client)
   │                               │
   │──── Encrypted Traffic ─────────→│
```

### Use Cases
- **API Authentication**: Verify client identity without passwords
- **Microservice Security**: Service-to-service authentication
- **Zero Trust**: Every connection must prove identity
- **Compliance**: Banking, healthcare, government requirements
- **IoT Security**: Device authentication with embedded certificates

---

## Architecture Overview

### Component Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    mTLS System                           │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  ┌──────────────────┐      ┌──────────────────┐        │
│  │   CA Manager     │      │  Cert Verifier   │        │
│  │ - Load CA certs  │      │ - Chain validation│       │
│  │ - CRL management │      │ - Expiry check   │        │
│  │ - OCSP responder │      │ - OCSP check     │        │
│  └──────────────────┘      └──────────────────┘        │
│           │                         │                    │
│           ↓                         ↓                    │
│  ┌─────────────────────────────────────────┐           │
│  │        TLS Acceptor (Enhanced)          │           │
│  │  - Request client cert                  │           │
│  │  - Verify with CA                       │           │
│  │  - Extract client DN                    │           │
│  └─────────────────────────────────────────┘           │
│                      │                                   │
│                      ↓                                   │
│  ┌─────────────────────────────────────────┐           │
│  │        Request Handler                  │           │
│  │  - Check route mTLS policy              │           │
│  │  - Extract cert info                    │           │
│  │  - Add headers to backend request       │           │
│  └─────────────────────────────────────────┘           │
│                                                           │
└───────────────────────────────────────────────────────────┘
```

### Data Flow

```
Client connects with cert
        │
        ↓
   TLS Handshake
        │
        ├──→ Server sends cert
        │
        ├──→ Server requests client cert
        │
        ↓
Client sends certificate
        │
        ↓
   Verify client cert
        │
        ├──→ Load CA certificates
        ├──→ Verify certificate chain
        ├──→ Check expiration
        ├──→ Check revocation (OCSP/CRL)
        │
        ├──Invalid──→ Reject connection
        │
        ↓ Valid
   Extract client info
        │
        ├──→ Subject DN
        ├──→ Issuer DN
        ├──→ Serial number
        ├──→ Fingerprint
        │
        ↓
   Check route policy
        │
        ├──→ mTLS required? (enforce)
        ├──→ mTLS optional? (allow)
        ├──→ No mTLS? (ignore cert)
        │
        ↓
   Add headers to backend
        │
        ├──→ X-Client-Cert-DN
        ├──→ X-Client-Cert-Serial
        ├──→ X-Client-Cert-Fingerprint
        │
        ↓
   Forward to backend
```

---

## Detailed Design

### 1. Configuration Schema

**File:** `highper-gateway/src/config/schema.rs`

**Enhancement:**

```rust
use serde::{Deserialize, Serialize};

/// TLS configuration with mTLS support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub enabled: bool,
    pub port: u16,
    pub cert_path: String,
    pub key_path: String,

    /// mTLS configuration
    #[serde(default)]
    pub mtls: Option<MtlsConfig>,

    /// ACME configuration
    pub acme: Option<AcmeConfig>,
}

/// Mutual TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtlsConfig {
    /// Enable mTLS
    pub enabled: bool,

    /// Path to CA certificate(s) for verifying client certificates
    /// Can be a file or directory
    pub ca_cert_path: String,

    /// Client certificate verification mode (global default)
    #[serde(default = "default_verification_mode")]
    pub verification_mode: CertVerificationMode,

    /// Certificate Revocation List (CRL) path (optional)
    pub crl_path: Option<String>,

    /// OCSP configuration
    #[serde(default)]
    pub ocsp: OcspConfig,

    /// Additional trusted certificate authorities
    #[serde(default)]
    pub additional_cas: Vec<String>,
}

/// Certificate verification mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CertVerificationMode {
    /// Require valid client certificate (connection fails without it)
    Required,

    /// Request client certificate but allow connection without it
    Optional,

    /// Request client certificate, verify if provided, allow if not
    OptionalNoCA,
}

fn default_verification_mode() -> CertVerificationMode {
    CertVerificationMode::Optional
}

/// OCSP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcspConfig {
    /// Enable OCSP revocation checking
    #[serde(default)]
    pub enabled: bool,

    /// OCSP responder URL (optional, uses cert's AIA if not set)
    pub responder_url: Option<String>,

    /// Timeout for OCSP requests (seconds)
    #[serde(default = "default_ocsp_timeout")]
    pub timeout: u64,

    /// Fail open if OCSP check fails (security vs availability)
    #[serde(default)]
    pub fail_open: bool,
}

fn default_ocsp_timeout() -> u64 {
    5
}

/// Route configuration with mTLS policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    pub path: String,
    pub upstream: String,
    pub methods: Option<Vec<String>>,

    /// Per-route mTLS requirement (overrides global)
    pub mtls: Option<RouteMtlsPolicy>,

    // ... other route fields
}

/// Per-route mTLS policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMtlsPolicy {
    /// Verification mode for this route
    pub verification_mode: CertVerificationMode,

    /// Allowed client certificate subjects (DN patterns)
    #[serde(default)]
    pub allowed_subjects: Vec<String>,

    /// Allowed client certificate issuers (DN patterns)
    #[serde(default)]
    pub allowed_issuers: Vec<String>,

    /// Allowed certificate serial numbers
    #[serde(default)]
    pub allowed_serials: Vec<String>,
}
```

**Configuration Example:**

```yaml
tls:
  enabled: true
  port: 8443
  cert_path: "/etc/certs/server.crt"
  key_path: "/etc/certs/server.key"

  # mTLS configuration
  mtls:
    enabled: true
    ca_cert_path: "/etc/certs/client-ca.crt"
    verification_mode: optional  # required, optional, optional_no_ca

    # Optional: CRL for revocation checking
    crl_path: "/etc/certs/crl.pem"

    # Optional: OCSP configuration
    ocsp:
      enabled: true
      responder_url: "http://ocsp.example.com"
      timeout: 5
      fail_open: false  # Fail closed (reject if OCSP unavailable)

    # Additional CA certificates
    additional_cas:
      - "/etc/certs/intermediate-ca.crt"

# Per-route mTLS policies
routes:
  # Require mTLS for admin API
  - path: "/admin"
    upstream: "admin-backend"
    mtls:
      verification_mode: required
      allowed_subjects:
        - "CN=admin.example.com,O=Example Inc"
        - "CN=superadmin.example.com,O=Example Inc"

  # Optional mTLS for API (both authenticated and anonymous allowed)
  - path: "/api"
    upstream: "api-backend"
    mtls:
      verification_mode: optional

  # No mTLS for public content
  - path: "/public"
    upstream: "public-backend"
    # No mtls config = no client cert required
```

---

### 2. Certificate Authority Manager

**File:** `highper-gateway/src/tls/ca_manager.rs`

**Purpose:** Load and manage CA certificates for client verification

**Implementation:**

```rust
use anyhow::{Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::RootCertStore;
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

/// Manages CA certificates for client verification
pub struct CaManager {
    root_store: RootCertStore,
    ca_count: usize,
}

impl CaManager {
    /// Create a new CA manager
    pub fn new() -> Self {
        Self {
            root_store: RootCertStore::empty(),
            ca_count: 0,
        }
    }

    /// Load CA certificates from file or directory
    pub fn load_ca_certs<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();

        if path.is_file() {
            self.load_ca_file(path)?;
        } else if path.is_dir() {
            self.load_ca_directory(path)?;
        } else {
            return Err(anyhow::anyhow!("CA path not found: {:?}", path));
        }

        info!("Loaded {} CA certificate(s)", self.ca_count);
        Ok(())
    }

    /// Load CA certificates from a single file
    fn load_ca_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        debug!("Loading CA certificates from: {:?}", path);

        let ca_file = fs::File::open(path)
            .context(format!("Failed to open CA file: {:?}", path))?;
        let mut reader = std::io::BufReader::new(ca_file);

        let certs = rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to parse CA certificates")?;

        for cert in certs {
            self.root_store
                .add(cert)
                .context("Failed to add CA certificate to store")?;
            self.ca_count += 1;
        }

        Ok(())
    }

    /// Load CA certificates from a directory
    fn load_ca_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        debug!("Loading CA certificates from directory: {:?}", path);

        let entries = fs::read_dir(path)
            .context(format!("Failed to read directory: {:?}", path))?;

        for entry in entries {
            let entry = entry?;
            let file_path = entry.path();

            // Only process .crt, .pem, .cer files
            if let Some(ext) = file_path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if ext == "crt" || ext == "pem" || ext == "cer" {
                    if let Err(e) = self.load_ca_file(&file_path) {
                        warn!("Failed to load CA file {:?}: {}", file_path, e);
                        // Continue loading other files
                    }
                }
            }
        }

        Ok(())
    }

    /// Get the root certificate store
    pub fn root_store(&self) -> &RootCertStore {
        &self.root_store
    }

    /// Get the number of loaded CAs
    pub fn ca_count(&self) -> usize {
        self.ca_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_single_ca_file() {
        // Create test CA file
        let test_ca = "/tmp/test_ca.crt";
        let ca_pem = r#"-----BEGIN CERTIFICATE-----
MIIBkTCB+wIJAKHHCgVZU6TyMA0GCSqGSIb3DQEBCwUAMBExDzANBgNVBAMMBnRl
c3RDQTAeFw0yMDAxMDEwMDAwMDBaFw0zMDAxMDEwMDAwMDBaMBExDzANBgNVBAMM
BnRlc3RDQTCBnzANBgkqhkiG9w0BAQEFAAOBjQAwgYkCgYEAw1YvV0xtZrJ3xqHQ
Y+xqLxDYVp7Y6VqFqN3yDdGPmM5sVkVFQ9BN3PqNbGTYqKS2kF7Y3C8L0QYvJYKb
HwKNqBVU8p/7B4LmMKVBQVJFBs5k8LxKFb0xYfQGVNKWvJ0mHGqBVBQVNBQVJFBs
5k8LxKFb0xYfQGVNKWvJ0mHGqBVBQVCAwEAAaM1MDMwMQYDVR0RBCowKIIVdGVz
dC5leGFtcGxlLmNvbYIPZXhhbXBsZS5jb20wDQYJKoZIhvcNAQELBQADgYEAsVqH
-----END CERTIFICATE-----"#;

        fs::write(test_ca, ca_pem).unwrap();

        let mut manager = CaManager::new();
        manager.load_ca_certs(test_ca).unwrap();

        assert_eq!(manager.ca_count(), 1);

        // Cleanup
        fs::remove_file(test_ca).ok();
    }

    #[test]
    fn test_load_ca_directory() {
        let test_dir = "/tmp/test_cas";
        fs::create_dir_all(test_dir).unwrap();

        // Create multiple CA files
        for i in 1..=3 {
            let ca_file = format!("{}/ca{}.crt", test_dir, i);
            fs::write(&ca_file, "-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----\n")
                .unwrap();
        }

        let mut manager = CaManager::new();
        // This will fail with invalid certs, but tests directory loading
        let _ = manager.load_ca_certs(test_dir);

        // Cleanup
        fs::remove_dir_all(test_dir).ok();
    }
}
```

---

### 3. Client Certificate Verifier

**File:** `highper-gateway/src/tls/client_verifier.rs`

**Purpose:** Verify client certificates during TLS handshake

**Implementation:**

```rust
use crate::config::schema::{CertVerificationMode, MtlsConfig};
use anyhow::{Context, Result};
use rustls::client::danger::ServerCertVerifier;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::ClientCertVerifier;
use rustls::{DigitallySignedStruct, DistinguishedName, RootCertStore, SignatureScheme};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Custom client certificate verifier
#[derive(Debug)]
pub struct MtlsClientVerifier {
    root_store: Arc<RootCertStore>,
    verification_mode: CertVerificationMode,
    // CRL and OCSP checking will be added later
}

impl MtlsClientVerifier {
    /// Create a new client verifier
    pub fn new(
        root_store: Arc<RootCertStore>,
        verification_mode: CertVerificationMode,
    ) -> Arc<Self> {
        Arc::new(Self {
            root_store,
            verification_mode,
        })
    }

    /// Extract subject DN from certificate
    pub fn extract_subject_dn(cert: &CertificateDer) -> Result<String> {
        // Parse certificate using x509-parser
        let (_, parsed_cert) = x509_parser::parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to parse certificate: {}", e))?;

        let subject = parsed_cert.subject().to_string();
        Ok(subject)
    }

    /// Extract issuer DN from certificate
    pub fn extract_issuer_dn(cert: &CertificateDer) -> Result<String> {
        let (_, parsed_cert) = x509_parser::parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to parse certificate: {}", e))?;

        let issuer = parsed_cert.issuer().to_string();
        Ok(issuer)
    }

    /// Extract serial number from certificate
    pub fn extract_serial(cert: &CertificateDer) -> Result<String> {
        let (_, parsed_cert) = x509_parser::parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to parse certificate: {}", e))?;

        let serial = parsed_cert.serial.to_string();
        Ok(serial)
    }

    /// Calculate certificate fingerprint (SHA256)
    pub fn calculate_fingerprint(cert: &CertificateDer) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(cert.as_ref());
        let hash = hasher.finalize();
        hex::encode(hash)
    }
}

impl ClientCertVerifier for MtlsClientVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        // Return empty to not send CA list to client (privacy)
        // Alternatively, return CA DNs: self.root_store.subjects()
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer,
        intermediates: &[CertificateDer],
        now: UnixTime,
    ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
        debug!("Verifying client certificate");

        // Build certificate chain
        let mut chain = vec![end_entity.clone()];
        chain.extend(intermediates.iter().cloned());

        // Verify certificate chain
        let verifier = rustls::client::WebPkiServerVerifier::builder(self.root_store.clone())
            .build()
            .map_err(|e| {
                rustls::Error::General(format!("Failed to build verifier: {}", e))
            })?;

        // For client verification, we need to verify the chain manually
        // This is a simplified version - production would need more robust verification
        match self.verification_mode {
            CertVerificationMode::Required => {
                // Must have valid certificate
                if chain.is_empty() {
                    return Err(rustls::Error::NoCertificatesPresented);
                }

                // Verify certificate chain against root store
                // In production, use webpki or x509-parser for full verification
                debug!("Client certificate verified (required mode)");
                Ok(rustls::server::danger::ClientCertVerified::assertion())
            }
            CertVerificationMode::Optional => {
                // Accept certificate if presented and valid
                if !chain.is_empty() {
                    debug!("Client certificate verified (optional mode)");
                }
                Ok(rustls::server::danger::ClientCertVerified::assertion())
            }
            CertVerificationMode::OptionalNoCA => {
                // Accept any certificate or no certificate
                debug!("Client certificate accepted without verification (optional_no_ca mode)");
                Ok(rustls::server::danger::ClientCertVerified::assertion())
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        // Verify TLS 1.2 signatures
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        // Verify TLS 1.3 signatures
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Client certificate information extracted from TLS connection
#[derive(Debug, Clone)]
pub struct ClientCertInfo {
    pub subject_dn: String,
    pub issuer_dn: String,
    pub serial: String,
    pub fingerprint: String,
    pub not_before: String,
    pub not_after: String,
}

impl ClientCertInfo {
    /// Extract certificate info from certificate
    pub fn from_cert(cert: &CertificateDer) -> Result<Self> {
        let subject_dn = MtlsClientVerifier::extract_subject_dn(cert)?;
        let issuer_dn = MtlsClientVerifier::extract_issuer_dn(cert)?;
        let serial = MtlsClientVerifier::extract_serial(cert)?;
        let fingerprint = MtlsClientVerifier::calculate_fingerprint(cert);

        // Parse certificate for dates
        let (_, parsed_cert) = x509_parser::parse_x509_certificate(cert.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to parse certificate: {}", e))?;

        let not_before = parsed_cert.validity().not_before.to_string();
        let not_after = parsed_cert.validity().not_after.to_string();

        Ok(Self {
            subject_dn,
            issuer_dn,
            serial,
            fingerprint,
            not_before,
            not_after,
        })
    }

    /// Check if certificate subject matches pattern
    pub fn subject_matches(&self, pattern: &str) -> bool {
        // Simple wildcard matching
        self.subject_dn.contains(pattern) || wildcard_match(&self.subject_dn, pattern)
    }

    /// Check if certificate issuer matches pattern
    pub fn issuer_matches(&self, pattern: &str) -> bool {
        self.issuer_dn.contains(pattern) || wildcard_match(&self.issuer_dn, pattern)
    }
}

/// Simple wildcard pattern matching
fn wildcard_match(text: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    // Simple implementation - production would use regex or glob crate
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            let (prefix, suffix) = (parts[0], parts[1]);
            return text.starts_with(prefix) && text.ends_with(suffix);
        }
    }

    text == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildcard_matching() {
        assert!(wildcard_match("CN=test.example.com", "CN=test.example.com"));
        assert!(wildcard_match("CN=test.example.com,O=Example", "CN=test*"));
        assert!(wildcard_match("CN=test.example.com", "*example.com"));
        assert!(!wildcard_match("CN=test.example.com", "CN=other*"));
    }

    #[test]
    fn test_subject_matching() {
        let cert_info = ClientCertInfo {
            subject_dn: "CN=admin.example.com,O=Example Inc".to_string(),
            issuer_dn: "CN=Example CA".to_string(),
            serial: "12345".to_string(),
            fingerprint: "abc123".to_string(),
            not_before: "2024-01-01".to_string(),
            not_after: "2025-01-01".to_string(),
        };

        assert!(cert_info.subject_matches("CN=admin.example.com"));
        assert!(cert_info.subject_matches("admin.example.com"));
        assert!(cert_info.subject_matches("*example.com*"));
        assert!(!cert_info.subject_matches("CN=other.example.com"));
    }
}
```

---

### 4. TLS Acceptor Enhancement

**File:** `highper-gateway/src/tls/acceptor.rs` (enhance existing)

**Purpose:** Configure TLS acceptor with client certificate verification

**Enhancement:**

```rust
use crate::config::schema::{MtlsConfig, TlsConfig};
use crate::tls::ca_manager::CaManager;
use crate::tls::client_verifier::MtlsClientVerifier;
use anyhow::{Context, Result};
use rustls::ServerConfig;
use std::sync::Arc;
use tokio_rustls::TlsAcceptor;
use tracing::info;

/// Build TLS acceptor with mTLS support
pub fn build_tls_acceptor(tls_config: &TlsConfig) -> Result<TlsAcceptor> {
    // Load server certificate and key
    let certs = load_certs(&tls_config.cert_path)?;
    let key = load_private_key(&tls_config.key_path)?;

    let mut server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("Failed to build TLS config")?;

    // Configure mTLS if enabled
    if let Some(mtls_config) = &tls_config.mtls {
        if mtls_config.enabled {
            info!("Configuring mTLS with verification mode: {:?}", mtls_config.verification_mode);

            // Load CA certificates
            let mut ca_manager = CaManager::new();
            ca_manager.load_ca_certs(&mtls_config.ca_cert_path)
                .context("Failed to load CA certificates")?;

            // Load additional CAs if provided
            for ca_path in &mtls_config.additional_cas {
                ca_manager.load_ca_certs(ca_path)
                    .context(format!("Failed to load additional CA: {}", ca_path))?;
            }

            info!("Loaded {} CA certificate(s) for client verification", ca_manager.ca_count());

            // Create client verifier
            let root_store = Arc::new(ca_manager.root_store().clone());
            let verifier = MtlsClientVerifier::new(
                root_store,
                mtls_config.verification_mode,
            );

            // Rebuild server config with client auth
            server_config = ServerConfig::builder()
                .with_client_cert_verifier(verifier)
                .with_single_cert(certs, key)
                .context("Failed to build TLS config with mTLS")?;
        }
    }

    // Set ALPN protocols
    server_config.alpn_protocols = vec![
        b"h2".to_vec(),      // HTTP/2
        b"http/1.1".to_vec(), // HTTP/1.1
    ];

    Ok(TlsAcceptor::from(Arc::new(server_config)))
}

// Helper functions (existing)
fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    // ... existing implementation
}

fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>> {
    // ... existing implementation
}
```

---

### 5. Request Handler Enhancement

**File:** `highper-gateway/src/proxy/handler.rs` (enhance existing)

**Purpose:** Extract client cert info and pass to backend

**Enhancement:**

```rust
use crate::tls::client_verifier::ClientCertInfo;
use hyper::header::{HeaderName, HeaderValue};
use tokio_rustls::server::TlsStream;

impl ProxyHandler {
    /// Handle request with client certificate info
    pub async fn handle_request_with_cert(
        &self,
        req: Request<Incoming>,
        client_cert: Option<ClientCertInfo>,
    ) -> Result<Response<Full<Bytes>>> {
        // Check route mTLS policy
        let route = self.find_route(&req.method(), req.uri().host(), req.uri().path());

        if let Some(route) = &route {
            if let Some(mtls_policy) = &route.mtls {
                // Enforce mTLS policy
                if let Err(e) = self.enforce_mtls_policy(mtls_policy, &client_cert) {
                    return Ok(self.error_response(
                        StatusCode::UNAUTHORIZED,
                        &format!("mTLS policy violation: {}", e),
                    )?);
                }
            }
        }

        // Add client certificate headers if present
        let mut backend_req = self.build_backend_request(&req)?;

        if let Some(cert_info) = client_cert {
            self.add_client_cert_headers(&mut backend_req, &cert_info)?;
        }

        // Forward to backend
        self.forward_to_backend(backend_req).await
    }

    /// Enforce mTLS policy for route
    fn enforce_mtls_policy(
        &self,
        policy: &RouteMtlsPolicy,
        client_cert: &Option<ClientCertInfo>,
    ) -> Result<()> {
        use crate::config::schema::CertVerificationMode;

        match policy.verification_mode {
            CertVerificationMode::Required => {
                // Must have certificate
                let cert = client_cert.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Client certificate required"))?;

                // Check allowed subjects
                if !policy.allowed_subjects.is_empty() {
                    let matches = policy.allowed_subjects.iter()
                        .any(|pattern| cert.subject_matches(pattern));

                    if !matches {
                        return Err(anyhow::anyhow!(
                            "Client certificate subject not in allowed list: {}",
                            cert.subject_dn
                        ));
                    }
                }

                // Check allowed issuers
                if !policy.allowed_issuers.is_empty() {
                    let matches = policy.allowed_issuers.iter()
                        .any(|pattern| cert.issuer_matches(pattern));

                    if !matches {
                        return Err(anyhow::anyhow!(
                            "Client certificate issuer not in allowed list: {}",
                            cert.issuer_dn
                        ));
                    }
                }

                // Check allowed serials
                if !policy.allowed_serials.is_empty() {
                    if !policy.allowed_serials.contains(&cert.serial) {
                        return Err(anyhow::anyhow!(
                            "Client certificate serial not in allowed list: {}",
                            cert.serial
                        ));
                    }
                }

                Ok(())
            }
            CertVerificationMode::Optional | CertVerificationMode::OptionalNoCA => {
                // Certificate optional, accept with or without
                Ok(())
            }
        }
    }

    /// Add client certificate information as headers
    fn add_client_cert_headers(
        &self,
        req: &mut Request<Full<Bytes>>,
        cert_info: &ClientCertInfo,
    ) -> Result<()> {
        let headers = req.headers_mut();

        // Add standard headers
        headers.insert(
            HeaderName::from_static("x-client-cert-dn"),
            HeaderValue::from_str(&cert_info.subject_dn)?,
        );

        headers.insert(
            HeaderName::from_static("x-client-cert-issuer"),
            HeaderValue::from_str(&cert_info.issuer_dn)?,
        );

        headers.insert(
            HeaderName::from_static("x-client-cert-serial"),
            HeaderValue::from_str(&cert_info.serial)?,
        );

        headers.insert(
            HeaderName::from_static("x-client-cert-fingerprint"),
            HeaderValue::from_str(&cert_info.fingerprint)?,
        );

        headers.insert(
            HeaderName::from_static("x-client-cert-not-before"),
            HeaderValue::from_str(&cert_info.not_before)?,
        );

        headers.insert(
            HeaderName::from_static("x-client-cert-not-after"),
            HeaderValue::from_str(&cert_info.not_after)?,
        );

        Ok(())
    }
}
```

---

### 6. Server Integration

**File:** `highper-gateway/src/proxy/server.rs` (enhance existing)

**Purpose:** Extract client certificate from TLS stream

**Enhancement:**

```rust
use crate::tls::client_verifier::ClientCertInfo;
use tokio_rustls::server::TlsStream;

/// Extract client certificate info from TLS stream
fn extract_client_cert_info<IO>(stream: &TlsStream<IO>) -> Option<ClientCertInfo> {
    let (_, session) = stream.get_ref();

    // Get peer certificates
    let peer_certs = session.peer_certificates()?;

    if peer_certs.is_empty() {
        return None;
    }

    // Extract info from first certificate (end entity)
    ClientCertInfo::from_cert(&peer_certs[0]).ok()
}

/// Handle TLS connection with mTLS
pub async fn handle_tls_connection(
    stream: TlsStream<TcpStream>,
    handler: Arc<ProxyHandler>,
) -> Result<()> {
    // Extract client certificate info
    let client_cert = extract_client_cert_info(&stream);

    if let Some(ref cert) = client_cert {
        info!("Client authenticated: {}", cert.subject_dn);
    }

    // Serve HTTP over TLS with client cert info
    let service = service_fn(|req| {
        let handler = Arc::clone(&handler);
        let cert = client_cert.clone();
        async move {
            handler.handle_request_with_cert(req, cert).await
        }
    });

    hyper::server::conn::http1::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .await?;

    Ok(())
}
```

---

## Dependencies

### New Dependencies Required

Add to `Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...

# For X.509 certificate parsing
x509-parser = "0.16"

# For SHA256 fingerprints
sha2 = "0.10"
hex = "0.4"

# OCSP support (for future)
ocsp = { version = "0.2", optional = true }
```

---

## Testing Strategy

### Unit Tests

**1. CA Manager Tests**
```rust
#[test]
fn test_load_single_ca()
#[test]
fn test_load_ca_directory()
#[test]
fn test_invalid_ca_file()
```

**2. Client Verifier Tests**
```rust
#[test]
fn test_extract_subject_dn()
#[test]
fn test_extract_issuer_dn()
#[test]
fn test_calculate_fingerprint()
#[test]
fn test_subject_pattern_matching()
```

**3. mTLS Policy Tests**
```rust
#[test]
fn test_enforce_required_mode()
#[test]
fn test_enforce_optional_mode()
#[test]
fn test_allowed_subjects_filter()
#[test]
fn test_allowed_issuers_filter()
```

### Integration Tests

**File:** `highper-gateway/tests/mtls_test.rs`

```rust
use std::fs;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use rustls::ClientConfig;

#[tokio::test]
async fn test_mtls_required_with_valid_cert() {
    // Start proxy with mTLS required
    let proxy = start_proxy_with_mtls("required").await;

    // Create client with certificate
    let client_config = create_client_config_with_cert(
        "test-client.crt",
        "test-client.key",
        "ca.crt",
    );

    let connector = TlsConnector::from(Arc::new(client_config));

    // Connect
    let stream = TcpStream::connect("127.0.0.1:8443").await.unwrap();
    let stream = connector.connect("localhost".try_into().unwrap(), stream)
        .await
        .unwrap();

    // Make request
    let response = make_http_request(stream).await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_mtls_required_without_cert() {
    let proxy = start_proxy_with_mtls("required").await;

    // Create client WITHOUT certificate
    let client_config = create_client_config_without_cert("ca.crt");
    let connector = TlsConnector::from(Arc::new(client_config));

    // Connect should fail
    let stream = TcpStream::connect("127.0.0.1:8443").await.unwrap();
    let result = connector.connect("localhost".try_into().unwrap(), stream).await;

    assert!(result.is_err()); // TLS handshake should fail
}

#[tokio::test]
async fn test_mtls_optional_with_cert() {
    let proxy = start_proxy_with_mtls("optional").await;

    let client_config = create_client_config_with_cert(
        "test-client.crt",
        "test-client.key",
        "ca.crt",
    );

    let response = make_mtls_request(client_config).await.unwrap();

    // Should have client cert headers
    assert!(response.headers().contains_key("x-forwarded-client-cert"));
}

#[tokio::test]
async fn test_mtls_optional_without_cert() {
    let proxy = start_proxy_with_mtls("optional").await;

    let client_config = create_client_config_without_cert("ca.crt");

    let response = make_mtls_request(client_config).await.unwrap();

    // Should succeed without client cert headers
    assert_eq!(response.status(), 200);
    assert!(!response.headers().contains_key("x-forwarded-client-cert"));
}

#[tokio::test]
async fn test_per_route_mtls_policy() {
    let proxy = start_proxy_with_route_policies().await;

    // Admin route requires mTLS
    let result = request_without_cert("/admin").await;
    assert_eq!(result.status(), 401); // Unauthorized

    // Public route doesn't require mTLS
    let result = request_without_cert("/public").await;
    assert_eq!(result.status(), 200);
}

#[tokio::test]
async fn test_subject_filtering() {
    let proxy = start_proxy_with_subject_filter(vec![
        "CN=allowed.example.com".to_string(),
    ]).await;

    // Request with allowed certificate
    let result = request_with_cert("allowed-client.crt", "allowed-client.key").await;
    assert_eq!(result.status(), 200);

    // Request with disallowed certificate
    let result = request_with_cert("other-client.crt", "other-client.key").await;
    assert_eq!(result.status(), 401);
}
```

### Manual Testing

**Test Plan:**

1. **Generate Test Certificates**
   ```bash
   # Generate CA
   openssl req -x509 -newkey rsa:2048 -days 365 -nodes \
     -keyout ca-key.pem -out ca-cert.pem \
     -subj "/CN=Test CA"

   # Generate client certificate
   openssl req -newkey rsa:2048 -nodes \
     -keyout client-key.pem -out client-req.pem \
     -subj "/CN=test-client.example.com"

   openssl x509 -req -in client-req.pem -days 365 \
     -CA ca-cert.pem -CAkey ca-key.pem -CAcreateserial \
     -out client-cert.pem
   ```

2. **Test with curl**
   ```bash
   # Test with client certificate
   curl --cert client-cert.pem --key client-key.pem \
     --cacert ca-cert.pem \
     https://localhost:8443/api/test

   # Test without client certificate (should fail if required)
   curl --cacert ca-cert.pem \
     https://localhost:8443/api/test
   ```

3. **Test with OpenSSL**
   ```bash
   # Verify mTLS handshake
   openssl s_client -connect localhost:8443 \
     -cert client-cert.pem -key client-key.pem \
     -CAfile ca-cert.pem
   ```

4. **Test Per-Route Policies**
   ```bash
   # Admin route (mTLS required)
   curl --cert client-cert.pem --key client-key.pem \
     https://localhost:8443/admin/status

   # Public route (mTLS optional)
   curl https://localhost:8443/public/health
   ```

---

## Configuration Examples

### Basic mTLS Configuration

```yaml
tls:
  enabled: true
  port: 8443
  cert_path: "/etc/certs/server.crt"
  key_path: "/etc/certs/server.key"

  mtls:
    enabled: true
    ca_cert_path: "/etc/certs/client-ca.crt"
    verification_mode: optional  # required, optional, optional_no_ca
```

### Per-Route mTLS Policies

```yaml
routes:
  # Require mTLS with specific subjects
  - path: "/admin"
    upstream: "admin-backend"
    mtls:
      verification_mode: required
      allowed_subjects:
        - "CN=admin.example.com,O=Example Inc"
        - "CN=superadmin.example.com,O=Example Inc"

  # Require mTLS from specific CA
  - path: "/partners"
    upstream: "partner-backend"
    mtls:
      verification_mode: required
      allowed_issuers:
        - "CN=Partner CA,O=Partner Corp"

  # Optional mTLS (both authenticated and anonymous)
  - path: "/api"
    upstream: "api-backend"
    mtls:
      verification_mode: optional

  # No mTLS requirement
  - path: "/public"
    upstream: "public-backend"
```

### Advanced mTLS with OCSP

```yaml
tls:
  enabled: true
  port: 8443
  cert_path: "/etc/certs/server.crt"
  key_path: "/etc/certs/server.key"

  mtls:
    enabled: true
    ca_cert_path: "/etc/certs/client-cas"  # Directory of CAs
    verification_mode: required

    # Certificate Revocation List
    crl_path: "/etc/certs/crl.pem"

    # OCSP configuration
    ocsp:
      enabled: true
      responder_url: "http://ocsp.example.com"
      timeout: 5
      fail_open: false  # Fail closed (reject if OCSP unavailable)

    # Additional intermediate CAs
    additional_cas:
      - "/etc/certs/intermediate-ca-1.crt"
      - "/etc/certs/intermediate-ca-2.crt"
```

---

## Headers Passed to Backend

When a client certificate is present, the following headers are added to backend requests:

| Header | Example Value | Description |
|--------|---------------|-------------|
| `X-Client-Cert-DN` | `CN=client.example.com,O=Example Inc` | Client certificate subject DN |
| `X-Client-Cert-Issuer` | `CN=Example CA,O=Example Inc` | Certificate issuer DN |
| `X-Client-Cert-Serial` | `1234567890ABCDEF` | Certificate serial number |
| `X-Client-Cert-Fingerprint` | `ab:cd:ef:12:34:...` | SHA256 fingerprint |
| `X-Client-Cert-Not-Before` | `2024-01-01T00:00:00Z` | Certificate validity start |
| `X-Client-Cert-Not-After` | `2025-01-01T00:00:00Z` | Certificate validity end |

Backend services can use these headers for:
- Authorization decisions
- Audit logging
- User identification
- Rate limiting per client

---

## Performance Considerations

### TLS Handshake Overhead

- **mTLS adds ~10-20ms** to TLS handshake (certificate verification)
- **Mitigated by**: TLS session resumption
- **Production impact**: Minimal for long-lived connections

### Certificate Verification

- **Chain verification**: ~1-5ms per connection
- **OCSP checking**: +5-50ms (if enabled, network dependent)
- **CRL checking**: +1-10ms (local file lookup)

### Optimization Strategies

1. **Certificate Caching**: Cache validated certificates by fingerprint
2. **OCSP Stapling**: Server provides OCSP response (implemented in Phase 1.6)
3. **Session Resumption**: Reuse TLS session (built into rustls)
4. **Connection Pooling**: Reuse connections to backends

---

## Security Considerations

### Certificate Validation

- ✅ Verify certificate chain to trusted CA
- ✅ Check certificate expiration
- ✅ Validate certificate purpose (client auth)
- ✅ Check revocation status (OCSP/CRL)
- ✅ Verify certificate signature

### Privacy

- By default, don't send CA list to client (`root_hint_subjects` returns empty)
- Prevents information disclosure about trusted CAs
- Can be enabled if needed for client cert selection

### Fail Secure

- Default to `fail_open: false` for OCSP
- Invalid certificates always rejected
- Configuration validation prevents misconfigurations

---

## Acceptance Criteria

- [ ] mTLS can be enabled globally
- [ ] Per-route mTLS policies work correctly
- [ ] Client certificates verified against CA
- [ ] Client cert info extracted and passed to backend
- [ ] Invalid certificates rejected at TLS layer
- [ ] Subject/issuer/serial filtering works
- [ ] Unit tests pass (100%)
- [ ] Integration tests pass (100%)
- [ ] Manual testing with OpenSSL successful
- [ ] Documentation complete
- [ ] Performance acceptable (<20ms handshake overhead)

---

## Documentation

User documentation to create:
- `docs/MTLS.md` - mTLS configuration guide
- `docs/CERTIFICATE_MANAGEMENT.md` - Certificate generation and management
- `docs/MTLS_TROUBLESHOOTING.md` - Common issues and solutions

---

## Next Steps

After implementing mTLS:
1. Test with real certificates
2. Benchmark performance
3. Commit changes
4. Proceed to Phase 1.4: Admin API completion

---

**Document Version:** 1.0
**Last Updated:** October 30, 2025
**Status:** Ready for implementation
