//! Configuration hot reload manager
//!
//! Handles reloading configuration without downtime by:
//! 1. Loading and validating new configuration
//! 2. Computing differences from current config
//! 3. Applying changes atomically
//! 4. Rolling back on failure

use super::{load_config, validate_config, Config, ConfigEvent, ConfigWatcher};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Minimum time between automatic config reloads (debounce period)
const RELOAD_DEBOUNCE: Duration = Duration::from_secs(1);

/// Configuration reload manager
pub struct ConfigReloader {
    /// Path to configuration file
    config_path: PathBuf,

    /// Current configuration (shared with runtime)
    config: Arc<RwLock<Config>>,

    /// File watcher
    watcher: Option<ConfigWatcher>,

    /// Event receiver from watcher
    event_rx: Option<mpsc::Receiver<ConfigEvent>>,

    /// Manual reload channel
    reload_tx: mpsc::Sender<ReloadTrigger>,
    reload_rx: mpsc::Receiver<ReloadTrigger>,

    /// Last reload time (for debouncing)
    last_reload: Option<Instant>,
}

/// Trigger for manual reload
#[derive(Debug, Clone, Copy)]
pub enum ReloadTrigger {
    /// SIGHUP signal received
    Signal,
    /// Manual reload requested
    Manual,
}

/// Result of a reload operation
#[derive(Debug)]
pub struct ReloadResult {
    /// Whether the reload succeeded
    pub success: bool,

    /// Error message if reload failed
    pub error: Option<String>,

    /// Number of routes changed
    pub routes_changed: usize,

    /// Number of upstreams changed
    pub upstreams_changed: usize,

    /// Whether TLS configuration changed
    pub tls_changed: bool,
}

impl ConfigReloader {
    /// Create a new config reloader
    pub fn new<P: AsRef<Path>>(config_path: P, initial_config: Config) -> Result<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        let config = Arc::new(RwLock::new(initial_config));

        // B11.2: bounded reload-trigger channel; capacity tunable via
        // HIGHPER_CONFIG_WATCHER_RELOAD_TRIGGER_CAPACITY (default 16).
        // try_current() so unit tests work without runtime_config::install.
        let capacity = crate::runtime_config::try_current()
            .map(|c| *c.config_watcher.reload_trigger_channel_capacity.get() as usize)
            .unwrap_or(16);
        let (reload_tx, reload_rx) = mpsc::channel(capacity.max(1));

        Ok(Self {
            config_path,
            config,
            watcher: None,
            event_rx: None,
            reload_tx,
            reload_rx,
            last_reload: None,
        })
    }

    /// Get a reference to the current configuration
    pub fn config(&self) -> Arc<RwLock<Config>> {
        self.config.clone()
    }

    /// Get the manual reload trigger sender. Returns a bounded `Sender`
    /// (per B11.2). Senders should use `try_send` (drop-on-full) since
    /// duplicate reload triggers fold to a single reload anyway.
    pub fn reload_trigger(&self) -> mpsc::Sender<ReloadTrigger> {
        self.reload_tx.clone()
    }

    /// Start watching for configuration changes
    pub fn start_watching(&mut self) -> Result<()> {
        let (mut watcher, event_rx) = ConfigWatcher::new(&self.config_path)?;
        watcher.watch()?;

        self.watcher = Some(watcher);
        self.event_rx = Some(event_rx);

        info!(
            "Started watching configuration file: {:?}",
            self.config_path
        );
        Ok(())
    }

    /// Run the reload loop
    pub async fn run(mut self) -> Result<()> {
        info!("Configuration reload manager started");

        let mut event_rx = self
            .event_rx
            .take()
            .context("Watcher not initialized - call start_watching() first")?;

        loop {
            tokio::select! {
                // File system event
                Some(event) = event_rx.recv() => {
                    match event {
                        ConfigEvent::Modified | ConfigEvent::Created => {
                            // Debounce: only reload if enough time has passed since last reload
                            let should_reload = match self.last_reload {
                                Some(last) => last.elapsed() >= RELOAD_DEBOUNCE,
                                None => true,
                            };

                            if should_reload {
                                info!("Configuration file changed, reloading...");
                                self.reload_config().await;
                                self.last_reload = Some(Instant::now());
                            } else {
                                debug!("Ignoring config change event (debounce period)");
                            }
                        }
                        ConfigEvent::Deleted => {
                            warn!("Configuration file deleted - keeping current configuration");
                        }
                        ConfigEvent::Error(e) => {
                            error!("File watcher error: {}", e);
                        }
                    }
                }

                // Manual reload trigger
                Some(trigger) = self.reload_rx.recv() => {
                    match trigger {
                        ReloadTrigger::Signal => {
                            info!("SIGHUP received, reloading configuration...");
                        }
                        ReloadTrigger::Manual => {
                            info!("Manual reload requested...");
                        }
                    }
                    self.reload_config().await;
                    self.last_reload = Some(Instant::now());
                }

                else => {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Reload configuration from file
    async fn reload_config(&self) -> ReloadResult {
        // Load new configuration
        let new_config = match load_config(&self.config_path) {
            Ok(config) => config,
            Err(e) => {
                error!("Failed to load configuration: {}", e);
                return ReloadResult {
                    success: false,
                    error: Some(format!("Failed to load: {}", e)),
                    routes_changed: 0,
                    upstreams_changed: 0,
                    tls_changed: false,
                };
            }
        };

        // Validate new configuration
        if let Err(e) = validate_config(&new_config) {
            error!("Configuration validation failed: {}", e);
            return ReloadResult {
                success: false,
                error: Some(format!("Validation failed: {}", e)),
                routes_changed: 0,
                upstreams_changed: 0,
                tls_changed: false,
            };
        }

        // Compute differences
        let current = self.config.read().await;
        let routes_changed = new_config.routes.len() != current.routes.len();
        let upstreams_changed = new_config.upstreams.len() != current.upstreams.len();
        let tls_changed = new_config.tls.is_some() != current.tls.is_some();
        drop(current);

        // Apply new configuration atomically
        let mut config_write = self.config.write().await;
        *config_write = new_config;
        drop(config_write);

        info!("Configuration reloaded successfully");
        debug!(
            "Routes changed: {}, Upstreams changed: {}, TLS changed: {}",
            routes_changed, upstreams_changed, tls_changed
        );

        ReloadResult {
            success: true,
            error: None,
            routes_changed: if routes_changed { 1 } else { 0 },
            upstreams_changed: if upstreams_changed { 1 } else { 0 },
            tls_changed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::time::{sleep, Duration};

    fn create_test_config() -> Config {
        Config {
            server: super::super::ServerConfig::default(),
            tls: None,
            upstreams: vec![],
            routes: vec![],
            observability: super::super::ObservabilityConfig::default(),
            websocket: crate::websocket::WebSocketConfig::default(),
            grpc: crate::grpc::GrpcConfig::default(),
            admin: None,
            cache: None,
            rate_limit: None,
            waf: None,
            graphql: None,
            webserver: None,
        }
    }

    #[tokio::test]
    async fn test_config_reloader_creation() {
        let temp_dir = std::env::temp_dir();
        let config_path = temp_dir.join("test_reload.yaml");

        let config = create_test_config();
        let reloader = ConfigReloader::new(&config_path, config);

        assert!(reloader.is_ok());
    }

    #[tokio::test]
    async fn test_reload_trigger() {
        let temp_dir = std::env::temp_dir();
        let config_path = temp_dir.join("test_trigger.yaml");

        let config = create_test_config();
        let reloader = ConfigReloader::new(&config_path, config).unwrap();

        let trigger = reloader.reload_trigger();
        assert!(trigger.try_send(ReloadTrigger::Manual).is_ok());
    }

    #[tokio::test]
    async fn test_shared_config_access() {
        let temp_dir = std::env::temp_dir();
        let config_path = temp_dir.join("test_shared.yaml");

        let config = create_test_config();
        let reloader = ConfigReloader::new(&config_path, config).unwrap();

        let shared_config = reloader.config();
        let config_read = shared_config.read().await;

        assert_eq!(config_read.server.bind, vec!["0.0.0.0:8080".to_string()]);
    }
}
