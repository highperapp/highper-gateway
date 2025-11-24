# Health Check Integration with Routes - Analysis & Implementation Plan

**Feature:** 2.2 Health Check Integration with Routes (P1)
**Date:** 2025-11-16
**Status:** Analysis Complete, Ready for Implementation

---

## 🎯 Objective

Integrate health checking into the HostnameRouter to ensure routes only direct traffic to healthy backend servers.

**Goals:**
- ✅ Automatic health checking for all upstreams
- ✅ Filter unhealthy backends during load balancing
- ✅ Seamless integration with existing routing system
- ✅ Zero-downtime server recovery
- ✅ Expose health status via API

---

## 📊 Current State Analysis

### Existing Health Check System

**File:** `src/proxy/health.rs`

```rust
pub struct Backend {
    pub server: ServerDef,
    healthy: AtomicBool,
    consecutive_successes: AtomicU32,
    consecutive_failures: AtomicU32,
    last_check: Mutex<Option<Instant>>,
    // ...
}

impl Backend {
    pub fn is_healthy(&self) -> bool;
    pub fn mark_success(&self, threshold: u32);
    pub fn mark_failure(&self, threshold: u32);
}

pub struct HealthChecker {
    backends: Vec<Arc<Backend>>,
    config: ActiveHealthCheckConfig,
    client: Client,
    // ...
}

impl HealthChecker {
    pub async fn start(&mut self);  // Background task
    async fn check_backend(&self, backend: &Backend);
}
```

**Features:**
- Active health checking (periodic HTTP requests)
- Passive health checking (track actual request failures)
- Configurable thresholds for marking healthy/unhealthy
- Background task running periodic checks

### Current UpstreamConfig

**File:** `src/gateway/routing/types.rs:160-172`

```rust
pub struct UpstreamConfig {
    pub servers: Vec<String>,
    pub algorithm: String,
    pub health_check: Option<HealthCheckConfig>,  // ← Already exists!
}

pub struct HealthCheckConfig {
    pub interval_secs: u64,
    pub timeout_secs: u64,
    pub path: String,
    pub expected_status: u16,
}
```

### Current HostnameRouter

**File:** `src/gateway/routing/mod.rs:60-70`

```rust
pub struct HostnameRouter {
    hosts: Arc<DashMap<String, Arc<HostRoutes>>>,
    upstreams: Arc<DashMap<String, UpstreamConfig>>,  // ← Added recently
    reload_handle: Arc<RwLock<Option<ReloadHandle>>>,
}
```

**Missing:**
- ❌ No health checker instances
- ❌ No backend health state tracking
- ❌ Load balancing doesn't filter unhealthy servers
- ❌ No health check background tasks

---

## 🏗️ Proposed Architecture

### 1. Enhanced UpstreamState

Create a new `UpstreamState` struct to manage upstream health:

```rust
/// Runtime state for an upstream (backends + health tracking)
pub struct UpstreamState {
    /// Upstream name
    name: String,

    /// Backend servers with health tracking
    backends: Vec<Arc<Backend>>,

    /// Load balancing algorithm
    algorithm: LoadBalancingAlgorithm,

    /// Health checker (if enabled)
    health_checker: Option<Arc<Mutex<HealthChecker>>>,

    /// Round-robin counter
    next_index: AtomicUsize,
}

impl UpstreamState {
    /// Create from config
    pub fn from_config(name: String, config: UpstreamConfig, client: Client) -> Self;

    /// Select a healthy backend server
    pub fn select_healthy_backend(&self) -> Option<Arc<Backend>>;

    /// Start health checking
    pub async fn start_health_checks(&self);

    /// Stop health checking
    pub async fn stop_health_checks(&self);

    /// Get all backends with status
    pub fn get_backends_status(&self) -> Vec<BackendStatus>;
}
```

### 2. Update HostnameRouter

```rust
pub struct HostnameRouter {
    hosts: Arc<DashMap<String, Arc<HostRoutes>>>,
    upstreams: Arc<DashMap<String, UpstreamConfig>>,  // Config
    upstream_states: Arc<DashMap<String, Arc<UpstreamState>>>,  // NEW: Runtime state
    reload_handle: Arc<RwLock<Option<ReloadHandle>>>,
    client: Client,  // NEW: For health checks
}

impl HostnameRouter {
    /// Add upstream with health checking
    pub async fn add_upstream_with_health(
        &self,
        name: String,
        config: UpstreamConfig,
    ) -> Result<()> {
        // 1. Store config
        self.upstreams.insert(name.clone(), config.clone());

        // 2. Create state with backends
        let state = UpstreamState::from_config(name.clone(), config, self.client.clone());

        // 3. Start health checks if configured
        if state.health_checker.is_some() {
            state.start_health_checks().await;
        }

        // 4. Store state
        self.upstream_states.insert(name, Arc::new(state));

        Ok(())
    }

    /// Select healthy backend for route
    pub fn select_backend_for_route(&self, upstream_name: &str) -> Option<String> {
        self.upstream_states
            .get(upstream_name)?
            .select_healthy_backend()
            .map(|backend| backend.server.url.clone())
    }
}
```

### 3. Integration with Handler

**File:** `src/proxy/handler.rs`

Current code at line ~365:
```rust
if let Some(upstream) = self.upstreams.get(&upstream_name) {
    if let Some(backend_url) = upstream.select_backend(client_ip.as_deref()) {
        // Forward request
    }
}
```

Updated code:
```rust
// Use hostname_router to get healthy backend
if let Some(backend_url) = self.hostname_router
    .as_ref()
    .and_then(|router| router.select_backend_for_route(&upstream_name))
{
    // Forward request to healthy backend
    let result = self.forward_request(req, &backend_url, &method).await;

    // Passive health tracking
    if let Some(router) = self.hostname_router.as_ref() {
        match result {
            Ok(_) => router.mark_backend_success(&upstream_name, &backend_url),
            Err(_) => router.mark_backend_failure(&upstream_name, &backend_url),
        }
    }

    result
}
```

---

## 🔄 Implementation Strategy

### Phase 1: Create UpstreamState (Foundation)

**File:** `src/gateway/routing/upstream_state.rs` (NEW)

- [x] Define `UpstreamState` struct
- [ ] Implement `from_config()` to create backends from server URLs
- [ ] Implement `select_healthy_backend()` with load balancing
- [ ] Add health checker integration
- [ ] Implement `start_health_checks()` and `stop_health_checks()`

**Dependencies:**
- Use existing `Backend` from `proxy/health.rs`
- Use existing `HealthChecker` from `proxy/health.rs`
- Convert `UpstreamConfig::health_check` → `ActiveHealthCheckConfig`

### Phase 2: Integrate with HostnameRouter

**File:** `src/gateway/routing/mod.rs`

- [ ] Add `upstream_states: DashMap<String, Arc<UpstreamState>>` field
- [ ] Add `client: Client` field for health checks
- [ ] Update constructor to accept `Client`
- [ ] Implement `add_upstream_with_health()`
- [ ] Implement `select_backend_for_route()`
- [ ] Implement `mark_backend_success()` / `mark_backend_failure()` for passive health
- [ ] Update `load_from_json()` to create upstream states
- [ ] Update `reload()` to restart health checks

### Phase 3: Update Handler Integration

**File:** `src/proxy/handler.rs`

- [ ] Pass `Client` to HostnameRouter during creation
- [ ] Update request forwarding to use `select_backend_for_route()`
- [ ] Add passive health tracking after each request
- [ ] Remove direct upstream selection (delegate to router)

### Phase 4: Testing & Validation

**Tests:**
- [ ] Unit test: UpstreamState backend selection
- [ ] Unit test: Health check filtering
- [ ] Integration test: Route to healthy backends only
- [ ] Integration test: Backend recovery after health check passes
- [ ] Integration test: Passive health tracking
- [ ] Load test: Performance with health checking enabled

---

## 📋 Detailed Implementation Checklist

### Step 1: Create UpstreamState Module

**File:** `src/gateway/routing/upstream_state.rs`

```rust
use crate::config::{ActiveHealthCheckConfig, ServerDef};
use crate::proxy::health::{Backend, HealthChecker};
use crate::proxy::Client;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;

pub struct UpstreamState {
    name: String,
    backends: Vec<Arc<Backend>>,
    algorithm: String,
    next_index: AtomicUsize,
    health_checker: Option<Arc<Mutex<HealthChecker>>>,
}

impl UpstreamState {
    pub fn from_config(
        name: String,
        config: UpstreamConfig,
        client: Client,
    ) -> Self {
        // Convert server URLs to Backend instances
        let backends: Vec<Arc<Backend>> = config.servers
            .iter()
            .map(|url| {
                Arc::new(Backend::new(ServerDef {
                    url: url.clone(),
                    weight: 1,
                }))
            })
            .collect();

        // Create health checker if configured
        let health_checker = config.health_check.map(|hc_config| {
            let active_config = ActiveHealthCheckConfig {
                enabled: true,
                interval: Duration::from_secs(hc_config.interval_secs),
                timeout: Duration::from_secs(hc_config.timeout_secs),
                path: hc_config.path,
                expected_status: StatusCode::from_u16(hc_config.expected_status).unwrap(),
                healthy_threshold: 2,
                unhealthy_threshold: 3,
            };

            Arc::new(Mutex::new(HealthChecker::new(
                backends.clone(),
                active_config,
                client.clone(),
                name.clone(),
                None, // No proxy_state for now
            )))
        });

        Self {
            name,
            backends,
            algorithm: config.algorithm,
            next_index: AtomicUsize::new(0),
            health_checker,
        }
    }

    pub fn select_healthy_backend(&self) -> Option<Arc<Backend>> {
        let healthy_backends: Vec<_> = self.backends
            .iter()
            .filter(|b| b.is_healthy())
            .collect();

        if healthy_backends.is_empty() {
            warn!("No healthy backends for upstream {}", self.name);
            return None;
        }

        // Simple round-robin for now
        let index = self.next_index.fetch_add(1, Ordering::Relaxed);
        Some(healthy_backends[index % healthy_backends.len()].clone())
    }

    pub async fn start_health_checks(&self) {
        if let Some(checker) = &self.health_checker {
            let mut checker = checker.lock();
            checker.start().await;
        }
    }

    pub async fn stop_health_checks(&self) {
        if let Some(checker) = &self.health_checker {
            let mut checker = checker.lock();
            checker.stop();
        }
    }
}
```

### Step 2: Export Module

**File:** `src/gateway/routing/mod.rs`

Add near top:
```rust
mod upstream_state;
pub use upstream_state::UpstreamState;
```

### Step 3: Update HostnameRouter Fields

**File:** `src/gateway/routing/mod.rs:60-70`

```rust
pub struct HostnameRouter {
    hosts: Arc<DashMap<String, Arc<HostRoutes>>>,
    upstreams: Arc<DashMap<String, UpstreamConfig>>,
    upstream_states: Arc<DashMap<String, Arc<UpstreamState>>>,  // NEW
    reload_handle: Arc<RwLock<Option<ReloadHandle>>>,
    client: Client,  // NEW
}
```

### Step 4: Update Constructor

```rust
impl HostnameRouter {
    pub fn new(client: Client) -> Self {  // NEW parameter
        Self {
            hosts: Arc::new(DashMap::new()),
            upstreams: Arc::new(DashMap::new()),
            upstream_states: Arc::new(DashMap::new()),  // NEW
            reload_handle: Arc::new(RwLock::new(None)),
            client,  // NEW
        }
    }
}
```

### Step 5: Add Backend Selection Methods

```rust
impl HostnameRouter {
    pub fn select_backend_for_route(&self, upstream_name: &str) -> Option<String> {
        self.upstream_states
            .get(upstream_name)?
            .select_healthy_backend()
            .map(|backend| backend.server.url.clone())
    }

    pub fn mark_backend_success(&self, upstream_name: &str, backend_url: &str) {
        if let Some(state) = self.upstream_states.get(upstream_name) {
            if let Some(backend) = state.backends.iter().find(|b| b.server.url == backend_url) {
                backend.mark_passive_success();
            }
        }
    }

    pub fn mark_backend_failure(&self, upstream_name: &str, backend_url: &str) {
        if let Some(state) = self.upstream_states.get(upstream_name) {
            if let Some(backend) = state.backends.iter().find(|b| b.server.url == backend_url) {
                backend.mark_passive_failure();
            }
        }
    }
}
```

---

## 🔍 Risk Assessment

### High Risk
- **Breaking existing upstreams:** Handler currently uses old upstream selection
  - *Mitigation:* Keep both paths, migrate gradually

### Medium Risk
- **Health check overhead:** Many upstreams = many background tasks
  - *Mitigation:* Shared task pool, configurable intervals

### Low Risk
- **Configuration compatibility:** health_check field already exists
  - *Benefit:* No config migration needed

---

## 📈 Success Metrics

- [ ] Routes only receive traffic to healthy backends
- [ ] Unhealthy backends automatically removed from rotation
- [ ] Backends automatically recover when health checks pass
- [ ] Zero-downtime during backend failures
- [ ] Health status visible via API
- [ ] Performance: <1ms overhead for health filtering

---

## 🚀 Next Steps

1. Create `upstream_state.rs` module
2. Implement `UpstreamState` struct and methods
3. Integrate with `HostnameRouter`
4. Update handler to use health-aware routing
5. Add tests
6. Update documentation

---

**Document Version:** 1.0
**Last Updated:** 2025-11-16
**Author:** Claude Code
**Status:** Ready for Implementation
