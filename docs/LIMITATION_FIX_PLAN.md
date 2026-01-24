# Highper Gateway - Limitation Fix Implementation Plan

**Version:** 1.0
**Date:** January 11, 2026
**License:** Apache 2.0
**Status:** Implementation Ready

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Known Limitations Analysis](#known-limitations-analysis)
3. [15 Scenario Gap Analysis](#15-scenario-gap-analysis)
4. [Implementation Plan](#implementation-plan)
5. [Priority Matrix](#priority-matrix)
6. [Implementation Phases](#implementation-phases)
7. [Testing Strategy](#testing-strategy)
8. [Success Criteria](#success-criteria)

---

## Executive Summary

### Current State

Based on `KNOWN_LIMITATIONS.md`, we have:
- **12 documented limitations** across 7 categories
- **1 P1 (Critical)**: Documentation (already fixed by KNOWN_LIMITATIONS.md)
- **6 P2 (High)**: Auth, TLS, Infrastructure
- **3 P3 (Medium)**: Service Discovery, Web Server, Windows
- **2 P4 (Low)**: Documentation examples

### Gap Analysis Results

After analyzing all 15 scenarios against known limitations:
- **Scenario 01-11**: ✅ **READY** - No blocking limitations
- **Scenario 12**: ⚠️ **NEEDS FIX** - Static discovery not implemented
- **Scenario 13**: ✅ **READY** - GraphQL fully functional
- **Scenario 14**: ⚠️ **NEEDS ENHANCEMENT** - Directory listing missing
- **Scenario 15**: ✅ **READY** - GeoIP fully functional

### Implementation Scope

**Must Fix (Blocking Production):**
1. Static service discovery (Scenario 12)
2. OAuth2 completion (Scenario 04 security enhancement)
3. OCSP fetcher hardening (Scenario 03 production readiness)

**Should Fix (Quality Improvements):**
4. Directory listing (Scenario 14 enhancement)
5. Certificate validation edge cases (Scenario 03, 09)
6. CRL checker enhancements (Scenario 09)
7. Cloud test cleanup automation (Infrastructure)

**Nice to Have:**
8. Documentation examples cleanup
9. Windows native support improvements

### Estimated Timeline

- **Phase 1 (Critical Fixes)**: 2-3 days
- **Phase 2 (Quality Improvements)**: 3-4 days
- **Phase 3 (Testing & Validation)**: 2 days
- **Total**: 7-9 days for complete implementation

---

## Known Limitations Analysis

### Limitation 1: Static Service Discovery Not Implemented

**Priority:** P3 → **Upgrade to P2** (blocks Scenario 12 alternative)
**Impact:** LOW → MEDIUM
**Location:** `highper-gateway/src/discovery/mod.rs`

#### Current Code

```rust
// src/discovery/mod.rs
DiscoveryBackend::Static => {
    Err(anyhow!("Static discovery not yet implemented"))
}
```

#### Impact on Scenarios

- **Scenario 12 (Microservices Discovery)**: Currently requires Consul/etcd. Static backend list would be useful for simple deployments.
- **All other scenarios**: Can use DNS or direct backend configuration (workaround available)

#### Implementation Plan

```rust
// Proposed implementation
DiscoveryBackend::Static => {
    // Read static backend list from config
    let backends = config.discovery.static_backends
        .ok_or_else(|| anyhow!("Static backends not configured"))?;

    // Convert to ServiceEndpoint format
    let endpoints: Vec<ServiceEndpoint> = backends
        .iter()
        .map(|addr| ServiceEndpoint {
            id: format!("static-{}", addr),
            address: addr.clone(),
            port: parse_port(addr),
            metadata: HashMap::new(),
            healthy: true,
        })
        .collect();

    Ok(endpoints)
}
```

#### Configuration Schema

```toml
[discovery]
type = "static"

[[discovery.static_backends]]
id = "backend-1"
address = "192.168.1.10"
port = 8080
metadata = { zone = "us-east-1a" }

[[discovery.static_backends]]
id = "backend-2"
address = "192.168.1.11"
port = 8080
metadata = { zone = "us-east-1b" }
```

#### Testing Strategy

```bash
# Unit tests
cargo test discovery::static

# Integration test
# Start gateway with static discovery
# Verify backend list is correct
# Verify health checks work
# Verify load balancing works
```

---

### Limitation 2: Directory Listing Not Implemented

**Priority:** P3
**Impact:** LOW
**Location:** `highper-gateway/src/webserver/static_files.rs`

#### Current Code

```rust
// src/webserver/static_files.rs
if path.is_dir() {
    return Err(anyhow!("Directory listing not yet implemented"));
}
```

#### Impact on Scenarios

- **Scenario 14 (Static + PHP-FPM)**: Cannot browse directories without index files
- **All other scenarios**: No impact (not using static file serving)

#### Implementation Plan

```rust
// Proposed implementation
use std::fs;

async fn serve_directory(path: &Path, config: &StaticFileConfig) -> Result<Response> {
    if !config.directory_listing {
        return Err(StatusCode::FORBIDDEN.into());
    }

    let entries = fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .collect::<Vec<_>>();

    // Sort entries: directories first, then files
    let mut dirs = vec![];
    let mut files = vec![];

    for entry in entries {
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            dirs.push(entry);
        } else {
            files.push(entry);
        }
    }

    dirs.sort_by_key(|e| e.file_name());
    files.sort_by_key(|e| e.file_name());

    // Generate HTML or JSON response
    match config.directory_listing_format {
        DirectoryListingFormat::Html => generate_html_listing(path, dirs, files),
        DirectoryListingFormat::Json => generate_json_listing(path, dirs, files),
    }
}

fn generate_html_listing(path: &Path, dirs: Vec<DirEntry>, files: Vec<DirEntry>) -> Result<Response> {
    let mut html = String::from("<!DOCTYPE html><html><head>");
    html.push_str("<title>Index of ");
    html.push_str(&path.display().to_string());
    html.push_str("</title>");
    html.push_str("<style>body{font-family:monospace;}</style></head><body>");
    html.push_str("<h1>Index of ");
    html.push_str(&path.display().to_string());
    html.push_str("</h1><hr><ul>");

    // Parent directory link
    if path.parent().is_some() {
        html.push_str("<li><a href=\"../\">../</a></li>");
    }

    // Directories
    for dir in dirs {
        let name = dir.file_name().to_string_lossy().to_string();
        html.push_str(&format!("<li><a href=\"{}/\">{}/</a></li>", name, name));
    }

    // Files
    for file in files {
        let name = file.file_name().to_string_lossy().to_string();
        let metadata = file.metadata()?;
        let size = metadata.len();
        html.push_str(&format!(
            "<li><a href=\"{}\">{}</a> ({} bytes)</li>",
            name, name, size
        ));
    }

    html.push_str("</ul><hr></body></html>");

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(html.into())?)
}

fn generate_json_listing(path: &Path, dirs: Vec<DirEntry>, files: Vec<DirEntry>) -> Result<Response> {
    let listing = DirectoryListing {
        path: path.display().to_string(),
        directories: dirs.iter().map(|d| d.file_name().to_string_lossy().to_string()).collect(),
        files: files.iter().map(|f| {
            let metadata = f.metadata().ok();
            FileInfo {
                name: f.file_name().to_string_lossy().to_string(),
                size: metadata.map(|m| m.len()),
                modified: metadata.and_then(|m| m.modified().ok()),
            }
        }).collect(),
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&listing)?.into())?)
}
```

#### Configuration Schema

```toml
[webserver.static]
directory_listing = true
directory_listing_format = "html"  # html or json
show_hidden_files = false
```

---

### Limitation 3: OAuth2 Implementation Incomplete

**Priority:** P2 (High)
**Impact:** MEDIUM
**Location:** `highper-gateway/src/gateway/auth/oauth2.rs`

#### Current TODOs

1. Token validation edge cases
2. Token refresh logic
3. PKCE flow support
4. Multi-provider support
5. Token revocation

#### Impact on Scenarios

- **Scenario 04 (API Gateway)**: OAuth2 can be used for authentication, but lacks advanced features
- **All scenarios with auth**: Basic OAuth2 works, advanced features missing

#### Implementation Plan

##### 3.1: Token Refresh Logic

```rust
// src/gateway/auth/oauth2.rs

pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse> {
    let token_url = self.config.token_url.clone();

    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", &self.config.client_id),
        ("client_secret", &self.config.client_secret),
    ];

    let client = reqwest::Client::new();
    let response = client
        .post(&token_url)
        .form(&params)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!("Token refresh failed: {}", response.status()));
    }

    let token_response: TokenResponse = response.json().await?;

    // Cache new token
    self.cache_token(&token_response).await?;

    Ok(token_response)
}

// Automatic token refresh middleware
pub async fn ensure_valid_token(&self, token: &str) -> Result<String> {
    // Check if token is expired
    if self.is_token_expired(token)? {
        // Extract refresh token from cache
        let refresh_token = self.get_refresh_token(token).await?;

        // Refresh token
        let new_token = self.refresh_token(&refresh_token).await?;

        Ok(new_token.access_token)
    } else {
        Ok(token.to_string())
    }
}
```

##### 3.2: PKCE Flow Support

```rust
// src/gateway/auth/oauth2.rs

use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

pub struct PKCEChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
}

pub fn generate_pkce_challenge() -> PKCEChallenge {
    // Generate random code_verifier (43-128 characters)
    let code_verifier: String = (0..64)
        .map(|_| {
            let idx = rand::random::<usize>() % 62;
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"[idx] as char
        })
        .collect();

    // Generate code_challenge = BASE64URL(SHA256(code_verifier))
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    let code_challenge = URL_SAFE_NO_PAD.encode(&hash);

    PKCEChallenge {
        code_verifier,
        code_challenge,
    }
}

pub fn build_authorize_url_with_pkce(&self, state: &str) -> Result<(String, PKCEChallenge)> {
    let pkce = generate_pkce_challenge();

    let mut url = Url::parse(&self.config.authorize_url)?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &self.config.client_id)
        .append_pair("redirect_uri", &self.config.redirect_uri)
        .append_pair("state", state)
        .append_pair("code_challenge", &pkce.code_challenge)
        .append_pair("code_challenge_method", "S256");

    Ok((url.to_string(), pkce))
}

pub async fn exchange_code_with_pkce(
    &self,
    code: &str,
    code_verifier: &str,
) -> Result<TokenResponse> {
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", &self.config.redirect_uri),
        ("client_id", &self.config.client_id),
        ("code_verifier", code_verifier),
    ];

    let client = reqwest::Client::new();
    let response = client
        .post(&self.config.token_url)
        .form(&params)
        .send()
        .await?;

    let token_response: TokenResponse = response.json().await?;
    Ok(token_response)
}
```

##### 3.3: Token Revocation

```rust
// src/gateway/auth/oauth2.rs

pub async fn revoke_token(&self, token: &str, token_type_hint: &str) -> Result<()> {
    let revocation_url = self.config.revocation_url
        .as_ref()
        .ok_or_else(|| anyhow!("Revocation URL not configured"))?;

    let params = [
        ("token", token),
        ("token_type_hint", token_type_hint),  // "access_token" or "refresh_token"
        ("client_id", &self.config.client_id),
        ("client_secret", &self.config.client_secret),
    ];

    let client = reqwest::Client::new();
    let response = client
        .post(revocation_url)
        .form(&params)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!("Token revocation failed: {}", response.status()));
    }

    // Remove from local cache
    self.remove_token_from_cache(token).await?;

    Ok(())
}
```

##### 3.4: Multi-Provider Support

```rust
// src/gateway/auth/oauth2.rs

#[derive(Debug, Clone)]
pub enum OAuth2Provider {
    Google,
    GitHub,
    Microsoft,
    Okta,
    Auth0,
    Custom(CustomProviderConfig),
}

impl OAuth2Provider {
    pub fn get_config(&self) -> OAuth2Config {
        match self {
            OAuth2Provider::Google => OAuth2Config {
                authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
                token_url: "https://oauth2.googleapis.com/token".to_string(),
                revocation_url: Some("https://oauth2.googleapis.com/revoke".to_string()),
                scopes: vec!["openid".to_string(), "email".to_string(), "profile".to_string()],
                ..Default::default()
            },
            OAuth2Provider::GitHub => OAuth2Config {
                authorize_url: "https://github.com/login/oauth/authorize".to_string(),
                token_url: "https://github.com/login/oauth/access_token".to_string(),
                scopes: vec!["user".to_string(), "repo".to_string()],
                ..Default::default()
            },
            OAuth2Provider::Microsoft => OAuth2Config {
                authorize_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
                token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
                scopes: vec!["openid".to_string(), "profile".to_string(), "email".to_string()],
                ..Default::default()
            },
            OAuth2Provider::Custom(config) => config.clone().into(),
            // ... other providers
        }
    }
}
```

#### Configuration Schema

```toml
[gateway.auth]
type = "oauth2"
provider = "google"  # google, github, microsoft, okta, auth0, custom
client_id = "your-client-id"
client_secret = "your-client-secret"
redirect_uri = "https://your-domain.com/callback"

# Advanced features
enable_pkce = true
enable_token_refresh = true
enable_token_revocation = true

# Token refresh settings
token_refresh_threshold = 300  # Refresh 5 minutes before expiry
auto_refresh = true

# Custom provider (if provider = "custom")
[gateway.auth.custom_provider]
authorize_url = "https://your-auth-server.com/oauth/authorize"
token_url = "https://your-auth-server.com/oauth/token"
revocation_url = "https://your-auth-server.com/oauth/revoke"
userinfo_url = "https://your-auth-server.com/oauth/userinfo"
```

---

### Limitation 4: OCSP Fetcher Needs Production Hardening

**Priority:** P2 (High)
**Impact:** LOW
**Location:** `highper-gateway/src/tls/ocsp_fetcher.rs`

#### Current TODOs

1. Advanced error handling
2. Retry logic with exponential backoff
3. OCSP stapling failure recovery
4. Caching strategy optimization
5. Multiple OCSP responder support

#### Impact on Scenarios

- **Scenario 03 (HTTPS/TLS)**: OCSP works but could be more robust
- **Scenario 09 (WAF + mTLS)**: Same as above

#### Implementation Plan

```rust
// src/tls/ocsp_fetcher.rs

use backoff::{ExponentialBackoff, backoff::Backoff};

pub struct OcspFetcher {
    client: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, CachedOcspResponse>>>,
    config: OcspConfig,
}

#[derive(Clone)]
pub struct OcspConfig {
    pub cache_duration: Duration,
    pub retry_max_attempts: usize,
    pub retry_initial_interval: Duration,
    pub retry_max_interval: Duration,
    pub retry_multiplier: f64,
    pub timeout: Duration,
    pub responder_urls: Vec<String>,  // Multiple responders
}

impl OcspFetcher {
    pub async fn fetch_ocsp_response_with_retry(
        &self,
        cert: &Certificate,
        issuer: &Certificate,
    ) -> Result<OcspResponse> {
        let mut backoff = ExponentialBackoff {
            initial_interval: self.config.retry_initial_interval,
            max_interval: self.config.retry_max_interval,
            multiplier: self.config.retry_multiplier,
            max_elapsed_time: Some(self.config.timeout),
            ..Default::default()
        };

        let mut last_error = None;

        for attempt in 1..=self.config.retry_max_attempts {
            match self.fetch_ocsp_response_once(cert, issuer).await {
                Ok(response) => {
                    tracing::info!("OCSP fetch succeeded on attempt {}", attempt);
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e);

                    if attempt < self.config.retry_max_attempts {
                        if let Some(duration) = backoff.next_backoff() {
                            tracing::warn!(
                                "OCSP fetch failed on attempt {}, retrying in {:?}: {}",
                                attempt, duration, last_error.as_ref().unwrap()
                            );
                            tokio::time::sleep(duration).await;
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("OCSP fetch failed after {} attempts", self.config.retry_max_attempts)))
    }

    async fn fetch_ocsp_response_once(
        &self,
        cert: &Certificate,
        issuer: &Certificate,
    ) -> Result<OcspResponse> {
        // Try multiple OCSP responders
        let responders = self.get_ocsp_responders(cert)?;

        let mut last_error = None;

        for responder_url in responders {
            match self.fetch_from_responder(&responder_url, cert, issuer).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = Some(e);
                    tracing::warn!("OCSP responder {} failed: {}", responder_url, last_error.as_ref().unwrap());
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All OCSP responders failed")))
    }

    fn get_ocsp_responders(&self, cert: &Certificate) -> Result<Vec<String>> {
        // 1. Try configured responders
        if !self.config.responder_urls.is_empty() {
            return Ok(self.config.responder_urls.clone());
        }

        // 2. Extract from certificate AIA extension
        let responders = extract_ocsp_urls_from_cert(cert)?;

        if responders.is_empty() {
            return Err(anyhow!("No OCSP responders found"));
        }

        Ok(responders)
    }

    async fn fetch_from_responder(
        &self,
        responder_url: &str,
        cert: &Certificate,
        issuer: &Certificate,
    ) -> Result<OcspResponse> {
        let request = build_ocsp_request(cert, issuer)?;

        let response = self.client
            .post(responder_url)
            .header("Content-Type", "application/ocsp-request")
            .body(request)
            .timeout(self.config.timeout)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("OCSP responder returned {}", response.status()));
        }

        let response_bytes = response.bytes().await?;
        let ocsp_response = parse_ocsp_response(&response_bytes)?;

        // Validate response
        validate_ocsp_response(&ocsp_response, cert, issuer)?;

        // Cache response
        self.cache_response(cert, &ocsp_response).await?;

        Ok(ocsp_response)
    }

    // Graceful degradation: serve stale OCSP response if fetch fails
    pub async fn get_ocsp_response_with_fallback(
        &self,
        cert: &Certificate,
        issuer: &Certificate,
    ) -> Option<OcspResponse> {
        // Try to fetch fresh response
        match self.fetch_ocsp_response_with_retry(cert, issuer).await {
            Ok(response) => Some(response),
            Err(e) => {
                tracing::error!("OCSP fetch failed: {}", e);

                // Fallback: try to get cached (possibly stale) response
                if let Some(cached) = self.get_cached_response(cert).await {
                    if cached.is_stale() {
                        tracing::warn!("Using stale OCSP response as fallback");
                    }
                    Some(cached.response)
                } else {
                    tracing::error!("No OCSP response available (fresh or cached)");
                    None
                }
            }
        }
    }
}
```

#### Configuration Schema

```toml
[tls.ocsp]
enabled = true
cache_duration = 3600  # seconds

# Retry configuration
retry_max_attempts = 5
retry_initial_interval = 1  # seconds
retry_max_interval = 30  # seconds
retry_multiplier = 2.0
timeout = 10  # seconds

# Multiple OCSP responders (optional, fallback to cert AIA)
responder_urls = [
    "http://ocsp.example.com",
    "http://ocsp2.example.com",
]

# Graceful degradation
allow_stale_responses = true
max_stale_age = 86400  # 24 hours
```

---

### Limitation 5: Certificate Validation Edge Cases

**Priority:** P2 (High)
**Impact:** LOW
**Location:** `highper-gateway/src/tls/cert_validator.rs`

#### Implementation Plan

```rust
// src/tls/cert_validator.rs

pub fn validate_certificate_chain(
    &self,
    cert: &Certificate,
    chain: &[Certificate],
    roots: &RootCertStore,
) -> Result<()> {
    // Standard validation
    self.validate_basic(cert, chain, roots)?;

    // Handle partial chains
    if chain.is_empty() {
        tracing::warn!("Certificate chain is empty, attempting to build chain");
        let built_chain = self.build_certificate_chain(cert, roots)?;
        return self.validate_basic(cert, &built_chain, roots);
    }

    // Handle cross-signed certificates
    if self.is_cross_signed(cert, chain)? {
        tracing::debug!("Certificate is cross-signed, validating alternative chains");
        return self.validate_cross_signed(cert, chain, roots);
    }

    Ok(())
}

fn build_certificate_chain(
    &self,
    cert: &Certificate,
    roots: &RootCertStore,
) -> Result<Vec<Certificate>> {
    // Try to build chain from local certificate store
    // or from AIA (Authority Information Access) extension
    let mut chain = vec![];
    let mut current = cert.clone();

    while !self.is_self_signed(&current)? {
        let issuer = self.fetch_issuer_cert(&current)?;
        chain.push(issuer.clone());
        current = issuer;

        // Prevent infinite loops
        if chain.len() > 10 {
            return Err(anyhow!("Certificate chain too long"));
        }
    }

    Ok(chain)
}

fn validate_cross_signed(
    &self,
    cert: &Certificate,
    chain: &[Certificate],
    roots: &RootCertStore,
) -> Result<()> {
    // Try all possible chain paths
    let mut validation_errors = vec![];

    for alternative_chain in self.enumerate_chain_alternatives(cert, chain)? {
        match self.validate_basic(cert, &alternative_chain, roots) {
            Ok(_) => return Ok(()),
            Err(e) => validation_errors.push(e),
        }
    }

    // All chains failed
    Err(anyhow!(
        "All certificate chain alternatives failed validation: {:?}",
        validation_errors
    ))
}
```

---

### Limitation 6: CRL Checker Enhancement

**Priority:** P2 (High)
**Impact:** LOW
**Location:** `highper-gateway/src/tls/crl_checker.rs`

#### Implementation Plan

```rust
// src/tls/crl_checker.rs

pub struct CrlChecker {
    cache: Arc<RwLock<HashMap<String, CachedCrl>>>,
    config: CrlConfig,
}

pub struct CrlConfig {
    pub enable_delta_crl: bool,
    pub cache_duration: Duration,
    pub max_cache_size: usize,
}

impl CrlChecker {
    pub async fn check_revocation_with_delta(
        &self,
        cert: &Certificate,
    ) -> Result<RevocationStatus> {
        // Fetch base CRL
        let base_crl = self.fetch_crl(cert).await?;

        // Check in base CRL
        if let Some(status) = base_crl.check_certificate(cert) {
            return Ok(status);
        }

        // If delta CRL enabled, fetch and check delta
        if self.config.enable_delta_crl {
            if let Some(delta_crl) = self.fetch_delta_crl(cert, &base_crl).await? {
                if let Some(status) = delta_crl.check_certificate(cert) {
                    return Ok(status);
                }
            }
        }

        Ok(RevocationStatus::Good)
    }

    async fn fetch_delta_crl(
        &self,
        cert: &Certificate,
        base_crl: &Crl,
    ) -> Result<Option<Crl>> {
        // Extract delta CRL URL from base CRL
        let delta_url = base_crl.delta_crl_distribution_point()
            .ok_or_else(|| anyhow!("No delta CRL distribution point"))?;

        // Fetch delta CRL
        let delta_crl = self.fetch_crl_from_url(&delta_url).await?;

        // Validate delta CRL against base CRL
        validate_delta_crl(&delta_crl, base_crl)?;

        Ok(Some(delta_crl))
    }

    // Enhanced caching with LRU eviction
    async fn cache_crl(&self, url: &str, crl: Crl) -> Result<()> {
        let mut cache = self.cache.write().await;

        // Check cache size
        if cache.len() >= self.config.max_cache_size {
            // Evict oldest entry (LRU)
            if let Some(oldest_key) = cache.iter()
                .min_by_key(|(_, v)| v.fetched_at)
                .map(|(k, _)| k.clone())
            {
                cache.remove(&oldest_key);
            }
        }

        cache.insert(url.to_string(), CachedCrl {
            crl,
            fetched_at: Instant::now(),
        });

        Ok(())
    }
}
```

---

### Limitation 7: Cloud Load Test Cleanup Automation

**Priority:** P2 (High)
**Impact:** MEDIUM
**Location:** `highper-gateway/tests/load/`

#### Implementation Plan

```bash
#!/bin/bash
# tests/load/cloud-test-runner.sh

set -euo pipefail

# Trap handlers for cleanup
INSTANCES=()
CLEANUP_DONE=false

cleanup() {
    if [ "$CLEANUP_DONE" = true ]; then
        return
    fi

    echo "🧹 Cleaning up cloud instances..."
    CLEANUP_DONE=true

    for instance_id in "${INSTANCES[@]}"; do
        echo "  Deleting instance: $instance_id"
        vultr-cli instance delete "$instance_id" --force || true
    done

    echo "✅ Cleanup complete"
}

# Trap on EXIT, INT, TERM, ERR
trap cleanup EXIT
trap cleanup INT
trap cleanup TERM
trap cleanup ERR

# Provision instances
provision_instance() {
    local name=$1
    echo "📦 Provisioning instance: $name"

    local instance_id=$(vultr-cli instance create \
        --region ewr \
        --plan vc2-1c-1gb \
        --os 387 \
        --label "$name" \
        --output json | jq -r '.id')

    INSTANCES+=("$instance_id")
    echo "  Instance ID: $instance_id"

    echo "$instance_id"
}

# Main test execution
main() {
    echo "🚀 Starting cloud load test"

    # Provision instances
    GATEWAY_INSTANCE=$(provision_instance "highper-gateway-test")
    BACKEND_INSTANCE=$(provision_instance "backend-test")
    LOADGEN_INSTANCE=$(provision_instance "loadgen-test")

    # Wait for instances to be ready
    echo "⏳ Waiting for instances to be ready..."
    wait_for_instances

    # Run tests
    echo "🧪 Running load tests..."
    run_load_tests || {
        echo "❌ Load tests failed"
        return 1
    }

    echo "✅ Load tests completed successfully"
}

# Run main (cleanup will be called automatically on exit)
main "$@"
```

---

## 15 Scenario Gap Analysis

### Scenario 01: Layer 4 TCP - Pure TCP Proxying

**Status:** ✅ **READY**

**Required Features:**
- ✅ TCP proxy mode
- ✅ Connection pooling
- ✅ Health checks (TCP connect)
- ✅ Load balancing algorithms

**No blockers or limitations**

---

### Scenario 02: Layer 7 HTTP - HTTP/1.1 Load Balancing

**Status:** ✅ **READY**

**Required Features:**
- ✅ HTTP/1.1 support
- ✅ Round-robin load balancing
- ✅ Connection pooling
- ✅ Health checks (HTTP GET)
- ✅ Circuit breaker

**No blockers or limitations**

---

### Scenario 03: HTTPS/TLS Termination

**Status:** ✅ **READY** (with minor enhancements recommended)

**Required Features:**
- ✅ TLS 1.2/1.3 support
- ✅ ACME/Let's Encrypt
- ✅ mTLS
- ✅ OCSP stapling

**Enhancements Recommended:**
- ⚠️ **Limitation 4**: OCSP fetcher hardening (P2)
- ⚠️ **Limitation 5**: Certificate validation edge cases (P2)

**Workaround:** Use standard certificates from trusted CAs. OCSP works reliably for 99% of cases.

---

### Scenario 04: API Gateway with Rate Limiting

**Status:** ✅ **READY** (with OAuth2 enhancement recommended)

**Required Features:**
- ✅ Rate limiting (token bucket, sliding window)
- ✅ API key authentication
- ✅ JWT authentication
- ⚠️ OAuth2 (basic flow works, advanced features incomplete)

**Enhancements Recommended:**
- ⚠️ **Limitation 3**: OAuth2 completion (P2) - PKCE, token refresh, revocation

**Workaround:** Use JWT or API key authentication (fully functional). Basic OAuth2 works for simple cases.

---

### Scenario 05: HTTP/3 QUIC

**Status:** ✅ **READY**

**Required Features:**
- ✅ HTTP/3 protocol support (Cloudflare quiche)
- ✅ QUIC transport
- ✅ TLS 1.3

**No blockers or limitations**

---

### Scenario 06: WebSocket Load Balancer

**Status:** ✅ **READY**

**Required Features:**
- ✅ WebSocket protocol support
- ✅ Connection persistence (sticky sessions)
- ✅ Ping/pong keepalive
- ✅ Load balancing

**No blockers or limitations**

---

### Scenario 07: gRPC Gateway

**Status:** ✅ **READY**

**Required Features:**
- ✅ gRPC protocol support
- ✅ HTTP/2
- ✅ Streaming (unary, server, client, bidirectional)
- ✅ Load balancing

**No blockers or limitations**

---

### Scenario 08: Database Load Balancer

**Status:** ✅ **READY**

**Required Features:**
- ✅ MySQL protocol support
- ✅ PostgreSQL protocol support
- ✅ Redis protocol support
- ✅ Connection pooling
- ✅ Read/write split

**No blockers or limitations**

---

### Scenario 09: WAF + mTLS

**Status:** ✅ **READY** (with minor enhancements recommended)

**Required Features:**
- ✅ WAF support (4 engines)
- ✅ mTLS client certificate validation
- ✅ Client certificate DN extraction
- ⚠️ CRL checking (basic, delta CRL not supported)

**Enhancements Recommended:**
- ⚠️ **Limitation 6**: CRL checker enhancement (P2) - delta CRL support

**Workaround:** Use OCSP instead of CRL (preferred and fully functional).

---

### Scenario 10: Hybrid Multi-Protocol

**Status:** ✅ **READY**

**Required Features:**
- ✅ HTTP + WebSocket + gRPC routing
- ✅ Path-based routing
- ✅ Protocol detection

**No blockers or limitations**

---

### Scenario 11: CDN Edge Caching

**Status:** ✅ **READY**

**Required Features:**
- ✅ In-memory L1 cache
- ✅ Redis L2 cache
- ✅ Multi-tier caching
- ✅ Cache key customization
- ✅ TTL management

**No blockers or limitations**

---

### Scenario 12: Microservices Discovery

**Status:** ⚠️ **NEEDS FIX** (minor - alternative available)

**Required Features:**
- ✅ Consul service discovery
- ✅ etcd service discovery
- ⚠️ Static discovery (not implemented)
- ✅ Circuit breaker
- ✅ Health checks

**Limitation:**
- ⚠️ **Limitation 1**: Static service discovery not implemented (P3 → P2)

**Workaround:** Use Consul or etcd (fully functional and production-ready). Static discovery only needed for very simple deployments.

**Impact:** LOW - Consul/etcd are better for production anyway.

---

### Scenario 13: GraphQL Gateway

**Status:** ✅ **READY**

**Required Features:**
- ✅ GraphQL protocol support
- ✅ Schema stitching
- ✅ Federation
- ✅ Query parsing
- ✅ Multiple backend support

**No blockers or limitations**

---

### Scenario 14: Static + PHP-FPM

**Status:** ⚠️ **ENHANCEMENT RECOMMENDED**

**Required Features:**
- ✅ Static file serving
- ✅ FastCGI protocol
- ✅ PHP-FPM support
- ⚠️ Directory listing (not implemented)

**Limitation:**
- ⚠️ **Limitation 2**: Directory listing not implemented (P3)

**Workaround:** Always provide index.html/index.php files. Use explicit file URLs.

**Impact:** LOW - Most production sites don't use directory listing for security reasons.

---

### Scenario 15: Geographic Load Balancing

**Status:** ✅ **READY**

**Required Features:**
- ✅ MaxMind GeoIP database support
- ✅ IP2Location support
- ✅ Geo-based routing
- ✅ Proximity-based load balancing
- ✅ Fallback region

**No blockers or limitations**

---

## Priority Matrix

### Must Fix (Blocking or High Value)

| # | Limitation | Priority | Impact | Affected Scenarios | Effort | Timeline |
|---|------------|----------|--------|-------------------|--------|----------|
| 1 | Static service discovery | P2 (upgraded) | MEDIUM | 12 | 1 day | Day 1 |
| 3 | OAuth2 completion | P2 | MEDIUM | 04 | 2 days | Day 1-2 |
| 4 | OCSP fetcher hardening | P2 | LOW | 03, 09 | 1 day | Day 2 |
| 7 | Cloud test cleanup | P2 | MEDIUM | Infrastructure | 0.5 day | Day 3 |

**Total:** 4.5 days

### Should Fix (Quality Improvements)

| # | Limitation | Priority | Impact | Affected Scenarios | Effort | Timeline |
|---|------------|----------|--------|-------------------|--------|----------|
| 2 | Directory listing | P3 | LOW | 14 | 1 day | Day 4 |
| 5 | Certificate validation | P2 | LOW | 03, 09 | 1 day | Day 4 |
| 6 | CRL checker | P2 | LOW | 09 | 1 day | Day 5 |

**Total:** 3 days

### Nice to Have (Low Priority)

| # | Limitation | Priority | Impact | Affected Scenarios | Effort | Timeline |
|---|------------|----------|--------|-------------------|--------|----------|
| 8 | Documentation examples | P4 | NONE | None | 0.5 day | Day 6 |
| 9 | Windows native support | P3 | LOW | Infrastructure | 2 days | Future |

**Total:** 0.5 days (excluding Windows)

---

## Implementation Phases

### Phase 1: Critical Fixes (Days 1-3)

**Goal:** Fix all P2 limitations that block production use or add high-value features

#### Day 1: Static Discovery + OAuth2 Part 1

**Morning:**
1. Implement static service discovery
   - Add `DiscoveryBackend::Static` implementation
   - Add configuration schema
   - Write unit tests
   - Update documentation

**Afternoon:**
2. Start OAuth2 enhancements
   - Implement token refresh logic
   - Add token expiry checking
   - Add automatic token refresh middleware

**Deliverable:** Static discovery working, OAuth2 refresh working

#### Day 2: OAuth2 Part 2 + OCSP Hardening

**Morning:**
3. Complete OAuth2 enhancements
   - Implement PKCE flow
   - Add multi-provider support
   - Implement token revocation
   - Write integration tests

**Afternoon:**
4. OCSP fetcher production hardening
   - Add exponential backoff retry logic
   - Add multiple OCSP responder support
   - Implement graceful degradation (stale responses)
   - Add comprehensive error handling

**Deliverable:** OAuth2 complete, OCSP robust

#### Day 3: Cloud Test Cleanup

**Morning:**
5. Automate cloud test cleanup
   - Add trap handlers for EXIT/INT/TERM/ERR
   - Implement cleanup on error
   - Add instance tracking
   - Test interrupted scenarios

**Afternoon:**
6. Integration testing of Phase 1 fixes
   - Test static discovery with load tests
   - Test OAuth2 PKCE flow
   - Test OCSP retry logic
   - Verify cleanup automation

**Deliverable:** All P2 fixes complete and tested

---

### Phase 2: Quality Improvements (Days 4-5)

**Goal:** Enhance features for better user experience

#### Day 4: Directory Listing + Cert Validation

**Morning:**
7. Implement directory listing
   - HTML listing format
   - JSON listing format
   - Configuration options
   - Security controls (hidden files)

**Afternoon:**
8. Enhanced certificate validation
   - Partial chain handling
   - Cross-signed certificate support
   - Automatic chain building
   - Alternative chain validation

**Deliverable:** Directory listing working, cert validation robust

#### Day 5: CRL Checker Enhancement

**Full Day:**
9. CRL checker improvements
   - Delta CRL support
   - Enhanced caching (LRU eviction)
   - Multiple CRL distribution points
   - Performance optimization

**Deliverable:** CRL checker production-ready

---

### Phase 3: Testing & Documentation (Days 6-7)

**Goal:** Comprehensive testing and documentation updates

#### Day 6: Integration Testing

**Full Day:**
10. Run all 15 scenario tests
    - Verify each scenario works with fixes
    - Run load tests
    - Run security tests
    - Validate health checks
    - Check metrics

**Deliverable:** All 15 scenarios passing

#### Day 7: Documentation & Release Prep

**Morning:**
11. Update documentation
    - Update KNOWN_LIMITATIONS.md
    - Update scenario docs
    - Update configuration examples
    - Update CHANGELOG.md

**Afternoon:**
12. Release preparation
    - Tag release candidate
    - Run final validation
    - Generate release notes
    - Prepare announcement

**Deliverable:** Ready for v1.1.0 release

---

## Testing Strategy

### Unit Tests

Each limitation fix must have comprehensive unit tests:

```rust
// Example: Static discovery unit tests
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_static_discovery_basic() {
        let config = DiscoveryConfig {
            backend: DiscoveryBackend::Static,
            static_backends: Some(vec![
                StaticBackend {
                    id: "backend-1".to_string(),
                    address: "192.168.1.10".to_string(),
                    port: 8080,
                    metadata: HashMap::new(),
                },
            ]),
            ..Default::default()
        };

        let discovery = Discovery::new(config);
        let endpoints = discovery.discover("test-service").await.unwrap();

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].address, "192.168.1.10");
    }

    #[tokio::test]
    async fn test_static_discovery_multiple_backends() {
        // Test with multiple backends
    }

    #[tokio::test]
    async fn test_static_discovery_health_checks() {
        // Test health check integration
    }
}
```

### Integration Tests

Run full scenario tests for each affected scenario:

```bash
# Scenario 12 with static discovery
cd tests/load
bash run-scenario-12-static-discovery.sh

# Expected: All tests pass, load balancing works
```

### Load Tests

Validate performance after fixes:

```bash
# Run performance benchmarks
cargo bench

# Run load tests for affected scenarios
for scenario in 03 04 09 12 14; do
    bash tests/load/run-scenario-$scenario.sh
done
```

### Security Tests

For TLS/auth fixes:

```bash
# Test OCSP stapling
openssl s_client -connect localhost:443 -status

# Test OAuth2 PKCE
curl -v https://localhost:8080/oauth/authorize?code_challenge=...

# Test certificate validation
openssl verify -CAfile ca.pem cert.pem
```

---

## Success Criteria

### Functional Criteria

- ✅ All 15 scenarios pass integration tests
- ✅ All new unit tests pass (100% coverage of new code)
- ✅ Load tests show no performance regression
- ✅ Security tests pass (TLS, OAuth2, WAF)

### Quality Criteria

- ✅ No new compiler warnings
- ✅ All TODOs resolved or documented
- ✅ Code reviewed by at least one other developer
- ✅ Documentation updated for all changes

### Production Readiness Criteria

- ✅ OCSP works reliably under failure scenarios
- ✅ OAuth2 works with major providers (Google, GitHub, Microsoft)
- ✅ Cloud tests clean up automatically (no cost leaks)
- ✅ Certificate validation handles edge cases

### Release Criteria

- ✅ All P2 limitations fixed
- ✅ KNOWN_LIMITATIONS.md updated
- ✅ CHANGELOG.md updated with detailed release notes
- ✅ Version bumped to v1.1.0
- ✅ Git tag created
- ✅ Release artifacts built for all platforms

---

## Risk Assessment

### Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| OAuth2 breaks existing flows | Low | High | Extensive testing, backward compatibility |
| OCSP changes cause TLS issues | Low | High | Feature flag, gradual rollout |
| Static discovery performance issues | Medium | Low | Load testing, benchmark comparisons |
| Certificate validation regression | Low | High | Comprehensive test suite with edge cases |

### Schedule Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| OAuth2 takes longer than 2 days | Medium | Medium | Prioritize PKCE, defer multi-provider |
| Integration testing reveals issues | Medium | High | Add 1 day buffer (Day 8) |
| Documentation incomplete | Low | Low | Documentation can be updated post-release |

---

## Implementation Checklist

### Pre-Implementation

- [ ] Review this plan with team
- [ ] Set up development branch: `limitation-fixes-v1.1.0`
- [ ] Create GitHub issues for each limitation
- [ ] Set up project board for tracking

### Phase 1 (Days 1-3)

- [ ] **Day 1**: Static discovery implementation
- [ ] **Day 1**: OAuth2 token refresh
- [ ] **Day 2**: OAuth2 PKCE + multi-provider
- [ ] **Day 2**: OCSP fetcher hardening
- [ ] **Day 3**: Cloud test cleanup automation
- [ ] **Day 3**: Phase 1 integration testing

### Phase 2 (Days 4-5)

- [ ] **Day 4**: Directory listing implementation
- [ ] **Day 4**: Certificate validation enhancement
- [ ] **Day 5**: CRL checker enhancement

### Phase 3 (Days 6-7)

- [ ] **Day 6**: Run all 15 scenario tests
- [ ] **Day 6**: Performance benchmarks
- [ ] **Day 7**: Documentation updates
- [ ] **Day 7**: Release preparation

### Post-Implementation

- [ ] Merge to main branch
- [ ] Tag v1.1.0 release
- [ ] Build release artifacts
- [ ] Publish release notes
- [ ] Update KNOWN_LIMITATIONS.md
- [ ] Close related GitHub issues

---

## Next Steps

1. **Review and Approve**: Team review of this implementation plan
2. **Resource Allocation**: Assign developer(s) for 7-9 day implementation
3. **Begin Phase 1**: Start with static discovery (highest priority for Scenario 12)
4. **Daily Standups**: Track progress and address blockers
5. **Release v1.1.0**: After all phases complete and tests pass

---

## Appendix: Configuration Examples

### Static Discovery Configuration

```toml
# Scenario 12 with static discovery
[discovery]
type = "static"

[[discovery.static_backends]]
id = "service-1"
address = "192.168.1.10"
port = 8080
metadata = { zone = "us-east-1a", version = "v1.2.0" }

[[discovery.static_backends]]
id = "service-2"
address = "192.168.1.11"
port = 8080
metadata = { zone = "us-east-1b", version = "v1.2.0" }

[health_check]
enabled = true
interval = "10s"
```

### Complete OAuth2 Configuration

```toml
# Scenario 04 with full OAuth2
[gateway.auth]
type = "oauth2"
provider = "google"
client_id = "your-client-id"
client_secret = "your-client-secret"
redirect_uri = "https://api.example.com/callback"

# Advanced features (NEW)
enable_pkce = true
enable_token_refresh = true
enable_token_revocation = true
token_refresh_threshold = 300
auto_refresh = true

scopes = ["openid", "email", "profile"]
```

### Enhanced OCSP Configuration

```toml
# Scenario 03 with robust OCSP
[tls.ocsp]
enabled = true
cache_duration = 3600

# Retry configuration (NEW)
retry_max_attempts = 5
retry_initial_interval = 1
retry_max_interval = 30
retry_multiplier = 2.0
timeout = 10

# Multiple responders (NEW)
responder_urls = [
    "http://ocsp.example.com",
    "http://ocsp2.example.com",
]

# Graceful degradation (NEW)
allow_stale_responses = true
max_stale_age = 86400
```

### Directory Listing Configuration

```toml
# Scenario 14 with directory listing
[webserver.static]
root = "/var/www/html"
index_files = ["index.php", "index.html", "index.htm"]

# Directory listing (NEW)
directory_listing = true
directory_listing_format = "html"
show_hidden_files = false
```

---

**Document Status:** Ready for Implementation
**Estimated Completion:** 7-9 days
**Target Release:** v1.1.0

---

**Questions or Feedback?**
Open an issue at: https://github.com/highperapp/highper-gateway/issues
