# Phase 1.2: Hot Reload Configuration - Implementation Specification

**Duration:** 2 weeks
**Priority:** High (Quick Win)
**Difficulty:** Medium
**Impact:** +6% configuration score

---

## Executive Summary

Implement hot reload capability to allow configuration changes without restarting the proxy. This includes file watching, validation, and zero-downtime reload mechanisms. This feature is critical for production deployments where downtime is unacceptable.

---

## Goals

### Primary Goals
1. Watch configuration file for changes
2. Validate new configuration before applying
3. Apply changes without dropping connections
4. Support SIGHUP signal for manual reload
5. Maintain backward compatibility

### Success Metrics
- Configuration changes applied within 1 second
- Zero dropped connections during reload
- Invalid configs rejected with clear errors
- +6% improvement in configuration score

---

## Architecture Overview

### Component Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    Hot Reload System                     │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  ┌──────────────┐        ┌─────────────────┐           │
│  │ File Watcher │───────→│ Config Validator │           │
│  │  (notify)    │        │                  │           │
│  └──────────────┘        └─────────────────┘           │
│         │                         │                      │
│         │                         ↓                      │
│         │                ┌─────────────────┐            │
│         │                │ Reload Manager  │            │
│         │                │                 │            │
│         │                └─────────────────┘            │
│         │                         │                      │
│         ↓                         ↓                      │
│  ┌──────────────┐        ┌─────────────────┐           │
│  │Signal Handler│        │  Runtime Updater│           │
│  │  (SIGHUP)    │───────→│  (Arc<RwLock>)  │           │
│  └──────────────┘        └─────────────────┘           │
│                                   │                      │
└───────────────────────────────────┼──────────────────────┘
                                    │
                     ┌──────────────┴──────────────┐
                     │                             │
                     ↓                             ↓
            ┌─────────────────┐         ┌─────────────────┐
            │  Proxy Server   │         │   Upstreams     │
            │  (keep running) │         │  (graceful swap)│
            └─────────────────┘         └─────────────────┘
```

### Data Flow

```
Configuration Change
        │
        ↓
    File System
        │
        ↓
  ┌─────────────┐
  │File Watcher │
  │  (inotify)  │
  └─────────────┘
        │
        ↓
  ┌─────────────┐
  │ Read File   │
  └─────────────┘
        │
        ↓
  ┌─────────────┐
  │Parse YAML   │
  └─────────────┘
        │
        ↓
  ┌─────────────┐
  │ Validate    │
  │ Schema      │
  └─────────────┘
        │
        ├──Invalid──→ Log Error + Keep Old Config
        │
        ↓ Valid
  ┌─────────────┐
  │ Compare     │
  │ Differences │
  └─────────────┘
        │
        ↓
  ┌─────────────┐
  │Apply Changes│
  │ Atomically  │
  └─────────────┘
        │
        ├────→ Update Routes
        ├────→ Update Upstreams
        ├────→ Update TLS Certs
        └────→ Update Middleware
```

---

## Detailed Design

### 1. File Watcher Module

**File:** `highper-gateway/src/config/watcher.rs`

**Purpose:** Monitor configuration file for changes using inotify (Linux), FSEvents (macOS), or polling (fallback)

**Implementation:**

```rust
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Configuration file watcher
pub struct ConfigWatcher {
    path: PathBuf,
    watcher: Option<RecommendedWatcher>,
    event_tx: mpsc::UnboundedSender<ConfigEvent>,
}

/// Events emitted by the config watcher
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// Configuration file was modified
    Modified,
    /// Configuration file was deleted
    Deleted,
    /// Configuration file was created
    Created,
    /// Error occurred while watching
    Error(String),
}

impl ConfigWatcher {
    /// Create a new config watcher
    pub fn new<P: AsRef<Path>>(
        path: P,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<ConfigEvent>)> {
        let path = path.as_ref().to_path_buf();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let tx = event_tx.clone();
        let watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                match result {
                    Ok(event) => {
                        debug!("File system event: {:?}", event);
                        match event.kind {
                            notify::EventKind::Modify(_) => {
                                let _ = tx.send(ConfigEvent::Modified);
                            }
                            notify::EventKind::Remove(_) => {
                                let _ = tx.send(ConfigEvent::Deleted);
                            }
                            notify::EventKind::Create(_) => {
                                let _ = tx.send(ConfigEvent::Created);
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        error!("Watch error: {}", e);
                        let _ = tx.send(ConfigEvent::Error(e.to_string()));
                    }
                }
            },
            Config::default(),
        )?;

        Ok((
            Self {
                path,
                watcher: Some(watcher),
                event_tx,
            },
            event_rx,
        ))
    }

    /// Start watching the configuration file
    pub fn watch(&mut self) -> anyhow::Result<()> {
        if let Some(watcher) = &mut self.watcher {
            watcher.watch(&self.path, RecursiveMode::NonRecursive)?;
            info!("Started watching config file: {:?}", self.path);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Watcher not initialized"))
        }
    }

    /// Stop watching
    pub fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(watcher) = &mut self.watcher {
            watcher.unwatch(&self.path)?;
            info!("Stopped watching config file");
        }
        self.watcher = None;
        Ok(())
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_file_modification_detection() {
        let temp_file = "/tmp/test_config.yaml";
        fs::write(temp_file, "test: value").unwrap();

        let (mut watcher, mut event_rx) = ConfigWatcher::new(temp_file).unwrap();
        watcher.watch().unwrap();

        // Modify file
        sleep(Duration::from_millis(100)).await;
        fs::write(temp_file, "test: new_value").unwrap();

        // Wait for event
        let event = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(event, ConfigEvent::Modified));

        // Cleanup
        fs::remove_file(temp_file).ok();
    }

    #[tokio::test]
    async fn test_file_deletion_detection() {
        let temp_file = "/tmp/test_config2.yaml";
        fs::write(temp_file, "test: value").unwrap();

        let (mut watcher, mut event_rx) = ConfigWatcher::new(temp_file).unwrap();
        watcher.watch().unwrap();

        // Delete file
        sleep(Duration::from_millis(100)).await;
        fs::remove_file(temp_file).unwrap();

        // Wait for event
        let event = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(event, ConfigEvent::Deleted));
    }
}
```

**Key Features:**
- Uses `notify` crate for cross-platform file watching
- Emits events via async channel
- Handles create, modify, delete events
- Automatic cleanup on drop
- Comprehensive error handling

---

### 2. Configuration Validator

**File:** `highper-gateway/src/config/validator.rs` (enhance existing)

**Purpose:** Validate new configuration before applying

**Enhancement:**

```rust
use super::schema::Config;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;

/// Validation errors
#[derive(Debug)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// Comprehensive configuration validator
pub struct ConfigValidator;

impl ConfigValidator {
    /// Validate a configuration thoroughly
    pub fn validate(config: &Config) -> Result<Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Validate server config
        Self::validate_server(&config.server, &mut errors);

        // Validate TLS config
        if let Some(tls) = &config.tls {
            Self::validate_tls(tls, &mut errors);
        }

        // Validate upstreams
        Self::validate_upstreams(&config.upstreams, &mut errors);

        // Validate routes
        Self::validate_routes(&config.routes, &config.upstreams, &mut errors);

        // Validate observability
        Self::validate_observability(&config.observability, &mut errors);

        // Cross-cutting validations
        Self::validate_consistency(config, &mut errors);

        Ok(errors)
    }

    fn validate_server(server: &ServerConfig, errors: &mut Vec<ValidationError>) {
        if server.port == 0 {
            errors.push(ValidationError {
                field: "server.port".to_string(),
                message: "Port must be greater than 0".to_string(),
            });
        }

        if server.port > 65535 {
            errors.push(ValidationError {
                field: "server.port".to_string(),
                message: "Port must be less than 65536".to_string(),
            });
        }

        if server.host.is_empty() {
            errors.push(ValidationError {
                field: "server.host".to_string(),
                message: "Host cannot be empty".to_string(),
            });
        }
    }

    fn validate_tls(tls: &TlsConfig, errors: &mut Vec<ValidationError>) {
        if tls.enabled {
            if tls.cert_path.is_empty() {
                errors.push(ValidationError {
                    field: "tls.cert_path".to_string(),
                    message: "Certificate path required when TLS is enabled".to_string(),
                });
            } else if !Path::new(&tls.cert_path).exists() {
                errors.push(ValidationError {
                    field: "tls.cert_path".to_string(),
                    message: format!("Certificate file not found: {}", tls.cert_path),
                });
            }

            if tls.key_path.is_empty() {
                errors.push(ValidationError {
                    field: "tls.key_path".to_string(),
                    message: "Key path required when TLS is enabled".to_string(),
                });
            } else if !Path::new(&tls.key_path).exists() {
                errors.push(ValidationError {
                    field: "tls.key_path".to_string(),
                    message: format!("Key file not found: {}", tls.key_path),
                });
            }
        }
    }

    fn validate_upstreams(upstreams: &[UpstreamConfig], errors: &mut Vec<ValidationError>) {
        let mut names = HashSet::new();

        for (idx, upstream) in upstreams.iter().enumerate() {
            // Check for duplicate names
            if !names.insert(&upstream.name) {
                errors.push(ValidationError {
                    field: format!("upstreams[{}].name", idx),
                    message: format!("Duplicate upstream name: {}", upstream.name),
                });
            }

            // Validate servers
            if upstream.servers.is_empty() {
                errors.push(ValidationError {
                    field: format!("upstreams[{}].servers", idx),
                    message: "At least one server required".to_string(),
                });
            }

            for (sidx, server) in upstream.servers.iter().enumerate() {
                if server.url.is_empty() {
                    errors.push(ValidationError {
                        field: format!("upstreams[{}].servers[{}].url", idx, sidx),
                        message: "Server URL cannot be empty".to_string(),
                    });
                }

                // Validate URL format
                if let Err(e) = url::Url::parse(&server.url) {
                    errors.push(ValidationError {
                        field: format!("upstreams[{}].servers[{}].url", idx, sidx),
                        message: format!("Invalid URL: {}", e),
                    });
                }
            }
        }
    }

    fn validate_routes(
        routes: &[RouteConfig],
        upstreams: &[UpstreamConfig],
        errors: &mut Vec<ValidationError>,
    ) {
        let upstream_names: HashSet<_> = upstreams.iter().map(|u| &u.name).collect();

        for (idx, route) in routes.iter().enumerate() {
            // Validate path
            if route.path.is_empty() {
                errors.push(ValidationError {
                    field: format!("routes[{}].path", idx),
                    message: "Path cannot be empty".to_string(),
                });
            }

            // Validate upstream reference
            if !upstream_names.contains(&route.upstream) {
                errors.push(ValidationError {
                    field: format!("routes[{}].upstream", idx),
                    message: format!("Upstream '{}' not found", route.upstream),
                });
            }

            // Validate path pattern
            if !route.path.starts_with('/') {
                errors.push(ValidationError {
                    field: format!("routes[{}].path", idx),
                    message: "Path must start with '/'".to_string(),
                });
            }
        }
    }

    fn validate_observability(obs: &ObservabilityConfig, errors: &mut Vec<ValidationError>) {
        if obs.metrics.enabled {
            if obs.metrics.port == 0 {
                errors.push(ValidationError {
                    field: "observability.metrics.port".to_string(),
                    message: "Metrics port must be greater than 0".to_string(),
                });
            }
        }
    }

    fn validate_consistency(config: &Config, errors: &mut Vec<ValidationError>) {
        // Check for port conflicts
        let mut ports = HashSet::new();

        if !ports.insert(config.server.port) {
            errors.push(ValidationError {
                field: "server.port".to_string(),
                message: "Port conflict detected".to_string(),
            });
        }

        if let Some(tls) = &config.tls {
            if tls.enabled && tls.port != 0 {
                if !ports.insert(tls.port) {
                    errors.push(ValidationError {
                        field: "tls.port".to_string(),
                        message: "Port conflict detected".to_string(),
                    });
                }
            }
        }

        if config.observability.metrics.enabled {
            if !ports.insert(config.observability.metrics.port) {
                errors.push(ValidationError {
                    field: "observability.metrics.port".to_string(),
                    message: "Port conflict detected".to_string(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_config() {
        let config = create_valid_test_config();
        let errors = ConfigValidator::validate(&config).unwrap();
        assert!(errors.is_empty(), "Valid config should have no errors");
    }

    #[test]
    fn test_invalid_port() {
        let mut config = create_valid_test_config();
        config.server.port = 0;

        let errors = ConfigValidator::validate(&config).unwrap();
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.field == "server.port"));
    }

    #[test]
    fn test_missing_upstream_reference() {
        let mut config = create_valid_test_config();
        config.routes[0].upstream = "nonexistent".to_string();

        let errors = ConfigValidator::validate(&config).unwrap();
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.field.contains("upstream")));
    }

    #[test]
    fn test_port_conflict() {
        let mut config = create_valid_test_config();
        config.server.port = 8080;
        config.observability.metrics.port = 8080;

        let errors = ConfigValidator::validate(&config).unwrap();
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.message.contains("conflict")));
    }
}
```

---

### 3. Reload Manager

**File:** `highper-gateway/src/config/reload.rs`

**Purpose:** Coordinate the reload process

**Implementation:**

```rust
use super::loader::ConfigLoader;
use super::schema::Config;
use super::validator::{ConfigValidator, ValidationError};
use super::watcher::{ConfigEvent, ConfigWatcher};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Manages configuration hot reloading
pub struct ReloadManager {
    config_path: PathBuf,
    current_config: Arc<RwLock<Config>>,
    reload_tx: mpsc::UnboundedSender<ReloadEvent>,
}

/// Events related to configuration reloading
#[derive(Debug)]
pub enum ReloadEvent {
    /// Configuration successfully reloaded
    Success { version: u64 },
    /// Configuration reload failed
    Failed { reason: String },
    /// Configuration validation failed
    ValidationFailed { errors: Vec<ValidationError> },
}

impl ReloadManager {
    /// Create a new reload manager
    pub fn new<P: AsRef<Path>>(
        config_path: P,
        initial_config: Config,
    ) -> (Self, mpsc::UnboundedReceiver<ReloadEvent>) {
        let (reload_tx, reload_rx) = mpsc::unbounded_channel();

        (
            Self {
                config_path: config_path.as_ref().to_path_buf(),
                current_config: Arc::new(RwLock::new(initial_config)),
                reload_tx,
            },
            reload_rx,
        )
    }

    /// Get reference to current config
    pub fn current_config(&self) -> Arc<RwLock<Config>> {
        Arc::clone(&self.current_config)
    }

    /// Start watching for configuration changes
    pub async fn start_watching(self: Arc<Self>) -> Result<()> {
        let (mut watcher, mut event_rx) = ConfigWatcher::new(&self.config_path)?;
        watcher.watch()?;

        info!("Hot reload enabled for: {:?}", self.config_path);

        // Spawn task to handle file events
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    ConfigEvent::Modified | ConfigEvent::Created => {
                        info!("Configuration file changed, reloading...");
                        if let Err(e) = self.reload_config().await {
                            error!("Failed to reload config: {}", e);
                            let _ = self.reload_tx.send(ReloadEvent::Failed {
                                reason: e.to_string(),
                            });
                        }
                    }
                    ConfigEvent::Deleted => {
                        warn!("Configuration file deleted, keeping current config");
                    }
                    ConfigEvent::Error(err) => {
                        error!("File watcher error: {}", err);
                    }
                }
            }
        });

        Ok(())
    }

    /// Manually trigger a configuration reload
    pub async fn reload_config(&self) -> Result<()> {
        debug!("Loading new configuration from: {:?}", self.config_path);

        // Load new config
        let new_config = ConfigLoader::load(&self.config_path)
            .context("Failed to load configuration file")?;

        // Validate new config
        let validation_errors = ConfigValidator::validate(&new_config)
            .context("Failed to validate configuration")?;

        if !validation_errors.is_empty() {
            let error_msg = validation_errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(", ");

            error!("Configuration validation failed: {}", error_msg);

            let _ = self.reload_tx.send(ReloadEvent::ValidationFailed {
                errors: validation_errors,
            });

            return Err(anyhow::anyhow!("Configuration validation failed"));
        }

        // Apply new config atomically
        let mut current = self.current_config.write().await;
        let old_version = current.version.unwrap_or(0);
        let new_version = old_version + 1;

        let mut new_config = new_config;
        new_config.version = Some(new_version);

        *current = new_config;
        drop(current); // Release lock

        info!("Configuration reloaded successfully (version {})", new_version);

        let _ = self.reload_tx.send(ReloadEvent::Success {
            version: new_version,
        });

        Ok(())
    }

    /// Get the current configuration version
    pub async fn current_version(&self) -> u64 {
        self.current_config.read().await.version.unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_manual_reload() {
        let temp_file = "/tmp/test_reload_config.yaml";
        let config = create_test_config();

        fs::write(temp_file, serde_yaml::to_string(&config).unwrap()).unwrap();

        let (manager, mut reload_rx) = ReloadManager::new(temp_file, config);
        let manager = Arc::new(manager);

        // Manually trigger reload
        manager.reload_config().await.unwrap();

        // Check for success event
        let event = tokio::time::timeout(Duration::from_secs(1), reload_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(event, ReloadEvent::Success { .. }));

        // Cleanup
        fs::remove_file(temp_file).ok();
    }

    #[tokio::test]
    async fn test_validation_failure() {
        let temp_file = "/tmp/test_invalid_config.yaml";

        // Write invalid config
        fs::write(temp_file, "invalid: yaml: content:").unwrap();

        let config = create_test_config();
        let (manager, mut reload_rx) = ReloadManager::new(temp_file, config);

        // Try to reload invalid config
        let result = manager.reload_config().await;
        assert!(result.is_err());

        // Cleanup
        fs::remove_file(temp_file).ok();
    }

    #[tokio::test]
    async fn test_version_incrementing() {
        let temp_file = "/tmp/test_version_config.yaml";
        let config = create_test_config();

        fs::write(temp_file, serde_yaml::to_string(&config).unwrap()).unwrap();

        let (manager, _) = ReloadManager::new(temp_file, config);

        let v1 = manager.current_version().await;
        manager.reload_config().await.unwrap();
        let v2 = manager.current_version().await;

        assert_eq!(v2, v1 + 1);

        // Cleanup
        fs::remove_file(temp_file).ok();
    }
}
```

---

### 4. Signal Handler

**File:** `highper-gateway/src/runtime/signals.rs`

**Purpose:** Handle SIGHUP for manual reload

**Implementation:**

```rust
use tokio::signal::unix::{signal, SignalKind};
use tracing::{error, info};

/// Signal handler for graceful shutdown and reload
pub struct SignalHandler;

impl SignalHandler {
    /// Listen for SIGHUP signal (reload)
    pub async fn handle_reload_signal<F, Fut>(reload_fn: F)
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = anyhow::Result<()>> + Send,
    {
        let mut sighup = signal(SignalKind::hangup()).expect("Failed to register SIGHUP handler");

        tokio::spawn(async move {
            loop {
                sighup.recv().await;
                info!("Received SIGHUP signal, reloading configuration...");

                match reload_fn().await {
                    Ok(()) => {
                        info!("Configuration reloaded successfully via SIGHUP");
                    }
                    Err(e) => {
                        error!("Failed to reload config via SIGHUP: {}", e);
                    }
                }
            }
        });
    }

    /// Listen for SIGTERM/SIGINT (shutdown)
    pub async fn wait_for_shutdown() {
        let mut sigterm = signal(SignalKind::terminate())
            .expect("Failed to register SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt())
            .expect("Failed to register SIGINT handler");

        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down...");
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down...");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_reload_signal() {
        let reloaded = Arc::new(AtomicBool::new(false));
        let reloaded_clone = Arc::clone(&reloaded);

        SignalHandler::handle_reload_signal(move || {
            let reloaded = Arc::clone(&reloaded_clone);
            async move {
                reloaded.store(true, Ordering::SeqCst);
                Ok(())
            }
        })
        .await;

        // Send SIGHUP to self
        unsafe {
            libc::kill(std::process::id() as i32, libc::SIGHUP);
        }

        // Wait for signal to be processed
        sleep(Duration::from_millis(100)).await;

        assert!(reloaded.load(Ordering::SeqCst));
    }
}
```

---

### 5. Runtime Integration

**File:** `highper-gateway/src/runtime/mod.rs` (enhance existing)

**Purpose:** Integrate hot reload into runtime

**Enhancement:**

```rust
use crate::config::reload::ReloadManager;
use crate::config::schema::Config;
use crate::runtime::signals::SignalHandler;
use std::sync::Arc;
use tracing::info;

pub async fn run_with_hot_reload(config_path: String, initial_config: Config) -> anyhow::Result<()> {
    // Create reload manager
    let (reload_manager, mut reload_rx) = ReloadManager::new(&config_path, initial_config.clone());
    let reload_manager = Arc::new(reload_manager);

    // Get shared config reference
    let shared_config = reload_manager.current_config();

    // Start file watcher
    let reload_manager_clone = Arc::clone(&reload_manager);
    reload_manager_clone.start_watching().await?;

    // Setup SIGHUP handler
    let reload_manager_clone = Arc::clone(&reload_manager);
    SignalHandler::handle_reload_signal(move || {
        let manager = Arc::clone(&reload_manager_clone);
        async move { manager.reload_config().await }
    })
    .await;

    // Spawn task to log reload events
    tokio::spawn(async move {
        while let Some(event) = reload_rx.recv().await {
            match event {
                crate::config::reload::ReloadEvent::Success { version } => {
                    info!("✅ Configuration version {} applied successfully", version);
                }
                crate::config::reload::ReloadEvent::Failed { reason } => {
                    error!("❌ Configuration reload failed: {}", reason);
                }
                crate::config::reload::ReloadEvent::ValidationFailed { errors } => {
                    error!("❌ Configuration validation failed:");
                    for err in errors {
                        error!("  - {}", err);
                    }
                }
            }
        }
    });

    // Start proxy server with shared config
    let server_handle = tokio::spawn(async move {
        crate::proxy::server::run_with_shared_config(shared_config).await
    });

    // Wait for shutdown signal
    SignalHandler::wait_for_shutdown().await;

    // Graceful shutdown
    info!("Shutting down gracefully...");
    server_handle.abort();

    Ok(())
}
```

---

### 6. Configuration Schema Update

**File:** `highper-gateway/src/config/schema.rs`

**Purpose:** Add version field to Config

**Enhancement:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub tls: Option<TlsConfig>,
    pub upstreams: Vec<UpstreamConfig>,
    pub routes: Vec<RouteConfig>,
    pub observability: ObservabilityConfig,
    pub websocket: crate::websocket::WebSocketConfig,
    pub grpc: crate::grpc::GrpcConfig,

    /// Configuration version (for tracking reloads)
    #[serde(skip)]
    pub version: Option<u64>,
}
```

---

## Dependencies

### New Dependencies Required

Add to `Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...

# For file watching
notify = "6.1"

# For URL validation
url = "2.5"
```

---

## Testing Strategy

### Unit Tests

**1. File Watcher Tests**
- Test file modification detection
- Test file deletion detection
- Test file creation detection
- Test error handling

**2. Validator Tests**
- Test valid configuration
- Test invalid ports
- Test missing upstream references
- Test port conflicts
- Test TLS certificate validation
- Test URL format validation

**3. Reload Manager Tests**
- Test manual reload
- Test validation failure handling
- Test version incrementing
- Test concurrent reloads

**4. Signal Handler Tests**
- Test SIGHUP handling
- Test SIGTERM/SIGINT handling

### Integration Tests

**File:** `highper-gateway/tests/hot_reload_test.rs`

```rust
use std::fs;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_hot_reload_end_to_end() {
    // Create initial config
    let config_path = "/tmp/test_hot_reload.yaml";
    let initial_config = r#"
server:
  host: "0.0.0.0"
  port: 8080

upstreams:
  - name: "backend1"
    servers:
      - url: "http://localhost:9001"

routes:
  - path: "/api"
    upstream: "backend1"
"#;

    fs::write(config_path, initial_config).unwrap();

    // Start proxy with hot reload
    let proxy_handle = tokio::spawn(async move {
        // Start proxy...
    });

    sleep(Duration::from_secs(1)).await;

    // Modify config
    let updated_config = r#"
server:
  host: "0.0.0.0"
  port: 8080

upstreams:
  - name: "backend1"
    servers:
      - url: "http://localhost:9001"
  - name: "backend2"
    servers:
      - url: "http://localhost:9002"

routes:
  - path: "/api"
    upstream: "backend1"
  - path: "/api/v2"
    upstream: "backend2"
"#;

    fs::write(config_path, updated_config).unwrap();

    // Wait for reload
    sleep(Duration::from_secs(2)).await;

    // Verify new route is available
    let response = reqwest::get("http://localhost:8080/api/v2/test")
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Cleanup
    proxy_handle.abort();
    fs::remove_file(config_path).ok();
}

#[tokio::test]
async fn test_invalid_config_rejected() {
    let config_path = "/tmp/test_invalid_reload.yaml";

    // Start with valid config
    let initial_config = create_valid_config();
    fs::write(config_path, initial_config).unwrap();

    let proxy_handle = tokio::spawn(async move {
        // Start proxy...
    });

    sleep(Duration::from_secs(1)).await;

    // Write invalid config
    let invalid_config = r#"
server:
  host: "0.0.0.0"
  port: 0  # Invalid port

upstreams: []  # No upstreams
routes: []
"#;

    fs::write(config_path, invalid_config).unwrap();

    // Wait for reload attempt
    sleep(Duration::from_secs(2)).await;

    // Verify proxy still works with old config
    let response = reqwest::get("http://localhost:8080/api/test")
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Cleanup
    proxy_handle.abort();
    fs::remove_file(config_path).ok();
}

#[tokio::test]
async fn test_sighup_reload() {
    let config_path = "/tmp/test_sighup_reload.yaml";
    let initial_config = create_valid_config();
    fs::write(config_path, initial_config).unwrap();

    let proxy_handle = tokio::spawn(async move {
        // Start proxy...
    });

    sleep(Duration::from_secs(1)).await;

    // Modify config
    fs::write(config_path, create_updated_config()).unwrap();

    // Send SIGHUP
    unsafe {
        libc::kill(std::process::id() as i32, libc::SIGHUP);
    }

    // Wait for reload
    sleep(Duration::from_secs(2)).await;

    // Verify reload occurred
    // (check logs or version endpoint)

    // Cleanup
    proxy_handle.abort();
    fs::remove_file(config_path).ok();
}
```

### Manual Testing

**Test Plan:**

1. **Basic Hot Reload**
   ```bash
   # Terminal 1: Start proxy
   ./target/release/highper-gateway config/example.yaml

   # Terminal 2: Make requests
   while true; do curl http://localhost:8080/api/test; sleep 1; done

   # Terminal 3: Modify config
   vim config/example.yaml
   # Save changes

   # Observe: No connection drops, new config applied
   ```

2. **Invalid Config Rejection**
   ```bash
   # Start proxy
   ./target/release/highper-gateway config/example.yaml

   # Break config
   echo "invalid: yaml: :" >> config/example.yaml

   # Observe: Error logged, old config retained
   ```

3. **SIGHUP Reload**
   ```bash
   # Start proxy
   ./target/release/highper-gateway config/example.yaml

   # Modify config
   vim config/example.yaml

   # Send SIGHUP
   kill -HUP $(pgrep highper-gateway)

   # Observe: Reload triggered
   ```

---

## Configuration Examples

### Enable Hot Reload

Hot reload is enabled by default when running with a config file:

```bash
highper-gateway config/production.yaml
```

### Configuration File

`config/production.yaml`:
```yaml
server:
  host: "0.0.0.0"
  port: 8080

# TLS configuration
tls:
  enabled: true
  port: 8443
  cert_path: "/etc/certs/server.crt"
  key_path: "/etc/certs/server.key"

# Upstreams can be changed without restart
upstreams:
  - name: "api-backend"
    servers:
      - url: "http://10.0.1.10:8000"
        weight: 2
      - url: "http://10.0.1.11:8000"
        weight: 1
    health_check:
      path: "/health"
      interval: 10

# Routes can be changed without restart
routes:
  - path: "/api/v1"
    upstream: "api-backend"
    methods: ["GET", "POST"]

observability:
  metrics:
    enabled: true
    port: 9090
```

### Reload Behavior

**What can be reloaded:**
- ✅ Routes (add/remove/modify)
- ✅ Upstreams (add/remove/modify)
- ✅ Backend servers (add/remove/modify)
- ✅ Health check intervals
- ✅ Load balancing algorithms
- ✅ Middleware configuration
- ✅ TLS certificate paths (certificates reloaded)
- ✅ Rate limiting configuration
- ✅ Auth configuration

**What cannot be reloaded (requires restart):**
- ❌ Server listen ports
- ❌ Server listen addresses
- ❌ Metrics port
- ❌ Log level (some loggers don't support runtime changes)

---

## Performance Considerations

### Reload Time

Expected reload times:
- Small config (<100 routes): **< 100ms**
- Medium config (100-1000 routes): **< 500ms**
- Large config (1000+ routes): **< 2s**

### Memory Usage

- RwLock allows multiple readers (proxy requests) simultaneously
- Write lock only held during config swap (~1ms)
- Minimal memory overhead: 2x config size during reload
- Old config dropped after last reader releases

### Connection Handling

```
During Reload:
┌──────────────────────────────────────┐
│  Existing Connections               │
│  ├─ Continue using old config       │
│  └─ No interruption                 │
├──────────────────────────────────────┤
│  New Connections                    │
│  ├─ Wait for write lock (~1ms)     │
│  └─ Use new config                  │
└──────────────────────────────────────┘
```

---

## Observability

### Metrics

Add to `highper-gateway/src/observability/metrics.rs`:

```rust
// Config reload metrics
pub static CONFIG_RELOAD_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "config_reload_total",
        "Total configuration reload attempts",
        &["status"]  // success, failed, validation_failed
    )
    .unwrap()
});

pub static CONFIG_RELOAD_DURATION: Lazy<Histogram> = Lazy::new(|| {
    register_histogram!(
        "config_reload_duration_seconds",
        "Configuration reload duration"
    )
    .unwrap()
});

pub static CONFIG_VERSION: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!("config_version", "Current configuration version").unwrap()
});
```

### Logging

```rust
info!("Configuration reloaded successfully (version {})", version);
error!("Configuration validation failed: {}", errors);
warn!("Configuration file deleted, keeping current config");
debug!("File system event: {:?}", event);
```

### Admin API Endpoint

Add endpoint to check reload status:

```
GET /api/config/version
{
  "version": 42,
  "last_reload": "2025-10-30T12:34:56Z",
  "status": "ok"
}
```

---

## Acceptance Criteria

- [ ] File watcher detects changes within 1 second
- [ ] Invalid configs are rejected with clear error messages
- [ ] Valid configs applied within 1 second
- [ ] Zero connection drops during reload
- [ ] SIGHUP triggers reload successfully
- [ ] Configuration version increments correctly
- [ ] Metrics track reload successes/failures
- [ ] Unit tests pass (100%)
- [ ] Integration tests pass (100%)
- [ ] Manual testing validates end-to-end workflow
- [ ] Documentation complete

---

## Documentation

### User Documentation

Create `docs/HOT_RELOAD.md`:

```markdown
# Hot Reload Configuration

## Overview

Highper Gateway supports hot reloading of configuration without restarting the server. This allows you to:
- Add/remove/modify routes
- Add/remove/modify upstreams
- Update backend servers
- Change middleware settings
- All without dropping connections

## How It Works

The proxy watches your configuration file for changes. When you modify and save the file, the proxy:
1. Detects the change (within 1 second)
2. Validates the new configuration
3. Applies changes if valid
4. Logs success or error

## Usage

### Automatic Reload

Simply edit your config file:
```bash
vim config/production.yaml
# Make changes
# Save file (:wq)
# Reload happens automatically
```

### Manual Reload (SIGHUP)

Send SIGHUP signal to trigger reload:
```bash
# Find process ID
ps aux | grep highper-gateway

# Send signal
kill -HUP <pid>

# Or use pkill
pkill -HUP highper-gateway
```

### Check Reload Status

View logs:
```bash
tail -f /var/log/highper-gateway.log

# Look for:
# ✅ Configuration version 42 applied successfully
# ❌ Configuration validation failed: ...
```

Check metrics:
```bash
curl http://localhost:9090/metrics | grep config_reload
```

## What Can Be Reloaded

✅ Routes
✅ Upstreams
✅ Backend servers
✅ Health checks
✅ Load balancing
✅ Middleware
✅ TLS certificates
✅ Rate limiting
✅ Authentication

## What Requires Restart

❌ Listen ports
❌ Listen addresses
❌ Metrics port

## Error Handling

If the new configuration is invalid:
- Error logged with details
- Old configuration retained
- Server continues running normally

## Best Practices

1. **Test config before deploying:**
   ```bash
   highper-gateway --validate config/production.yaml
   ```

2. **Use version control:**
   ```bash
   git diff config/production.yaml
   ```

3. **Monitor reload events:**
   ```bash
   tail -f /var/log/highper-gateway.log | grep reload
   ```

4. **Backup before changes:**
   ```bash
   cp config/production.yaml config/production.yaml.bak
   ```

## Troubleshooting

**Reload not happening?**
- Check file permissions
- Check log for errors
- Verify file watcher is working

**Configuration rejected?**
- Check validation errors in log
- Run manual validation
- Fix errors and save again

## Examples

See `config/hot-reload-example.yaml` for a complete example.
```

---

## Next Steps

After implementing hot reload:
1. Commit changes
2. Update main README with hot reload feature
3. Proceed to Phase 1.3: mTLS Support

---

**Document Version:** 1.0
**Last Updated:** October 30, 2025
**Status:** Ready for implementation
