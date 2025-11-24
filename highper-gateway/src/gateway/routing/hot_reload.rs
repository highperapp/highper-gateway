//! Hot reload support for zero-downtime configuration updates

use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time;
use tracing::{info, warn, error};

use super::HostRoutes;

/// Handle for hot reload background task
pub struct ReloadHandle {
    /// Path to configuration file
    config_path: PathBuf,

    /// Reference to router's hosts map
    hosts: Arc<DashMap<String, Arc<HostRoutes>>>,

    /// Background task handle
    task_handle: tokio::task::JoinHandle<()>,

    /// Shutdown signal
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl ReloadHandle {
    /// Create a new hot reload handle
    pub async fn new(
        config_path: String,
        interval_secs: u64,
        hosts: Arc<DashMap<String, Arc<HostRoutes>>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = PathBuf::from(config_path);

        // Verify file exists
        if !config_path.exists() {
            return Err(format!("Configuration file not found: {}", config_path.display()).into());
        }

        info!("Enabling hot reload for: {} (interval: {}s)", config_path.display(), interval_secs);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        // Get initial file modification time
        let initial_mtime = get_file_mtime(&config_path)?;

        // Spawn background task
        let task_handle = tokio::spawn(hot_reload_task(
            config_path.clone(),
            interval_secs,
            Arc::clone(&hosts),
            initial_mtime,
            shutdown_rx,
        ));

        Ok(Self {
            config_path,
            hosts,
            task_handle,
            shutdown_tx,
        })
    }

    /// Get configuration file path
    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    /// Trigger manual reload
    pub async fn reload_now(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Triggering manual reload");
        reload_routes(&self.config_path, &self.hosts).await
    }
}

impl Drop for ReloadHandle {
    fn drop(&mut self) {
        // Signal shutdown
        let _ = self.shutdown_tx.send(true);

        // Abort background task
        self.task_handle.abort();

        info!("Hot reload disabled");
    }
}

/// Background task for hot reload
async fn hot_reload_task(
    config_path: PathBuf,
    interval_secs: u64,
    hosts: Arc<DashMap<String, Arc<HostRoutes>>>,
    mut last_mtime: SystemTime,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    let mut interval = time::interval(Duration::from_secs(interval_secs));
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    info!("Hot reload task started");

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Check if file has been modified
                match get_file_mtime(&config_path) {
                    Ok(mtime) => {
                        if mtime > last_mtime {
                            info!("Configuration file modified, reloading routes");

                            match reload_routes(&config_path, &hosts).await {
                                Ok(()) => {
                                    last_mtime = mtime;
                                    info!("Routes reloaded successfully");
                                }
                                Err(e) => {
                                    error!("Failed to reload routes: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to check file modification time: {}", e);
                    }
                }
            }
            _ = shutdown_rx.changed() => {
                info!("Hot reload task shutting down");
                break;
            }
        }
    }
}

/// Get file modification time
fn get_file_mtime(path: &PathBuf) -> Result<SystemTime, std::io::Error> {
    let metadata = std::fs::metadata(path)?;
    metadata.modified()
}

/// Reload routes from configuration file
async fn reload_routes(
    config_path: &PathBuf,
    hosts: &Arc<DashMap<String, Arc<HostRoutes>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::fs;

    // Read configuration file
    let content = fs::read_to_string(config_path).await?;

    // Parse configuration
    let config: super::HostnameRoutesConfig = serde_json::from_str(&content)?;

    info!("Reloading {} hosts", config.hosts.len());

    // Build new hosts map
    let new_hosts = DashMap::new();

    for host_config in config.hosts {
        let hostname = host_config.hostname.clone();
        let host_routes = super::HostRoutes::new();

        for route_config in host_config.routes {
            let route = super::Route {
                name: route_config.name.clone(),
                upstream: route_config.upstream.clone(),
                methods: route_config.methods.clone(),
                timeout_ms: route_config.timeout_ms,
                middleware: route_config.middleware.clone(),
                metadata: route_config.metadata.clone(),
            };

            match route_config.path_match {
                super::PathMatch::Exact { path } => {
                    host_routes.add_exact(path, route);
                }
                super::PathMatch::Prefix { prefix } => {
                    host_routes.add_prefix(prefix, route).await;
                }
                super::PathMatch::Pattern { pattern } => {
                    if let Err(e) = host_routes.add_pattern(pattern, route).await {
                        warn!("Failed to add pattern route: {}", e);
                    }
                }
            }
        }

        new_hosts.insert(hostname, Arc::new(host_routes));
    }

    // Atomic swap: clear old routes and insert new ones
    hosts.clear();

    for entry in new_hosts.iter() {
        let hostname = entry.key().clone();
        let routes = Arc::clone(entry.value());
        hosts.insert(hostname, routes);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_file_mtime() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let path = PathBuf::from(temp_file.path());

        let mtime1 = get_file_mtime(&path).unwrap();

        // Wait a bit and modify file
        tokio::time::sleep(Duration::from_millis(100)).await;
        writeln!(temp_file, "modified").unwrap();
        temp_file.flush().unwrap();

        let mtime2 = get_file_mtime(&path).unwrap();

        assert!(mtime2 > mtime1);
    }

    #[tokio::test]
    async fn test_reload_handle_creation() {
        let json = r#"
        {
            "version": "1.0",
            "hosts": [
                {
                    "hostname": "test.example.com",
                    "routes": [
                        {
                            "name": "test",
                            "match_type": "exact",
                            "path": "/test",
                            "upstream": "backend",
                            "methods": ["GET"]
                        }
                    ]
                }
            ]
        }
        "#;

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", json).unwrap();
        temp_file.flush().unwrap();

        let hosts = Arc::new(DashMap::new());
        let config_path = temp_file.path().to_string_lossy().to_string();

        let handle = ReloadHandle::new(config_path, 10, Arc::clone(&hosts)).await;
        assert!(handle.is_ok());

        // Cleanup
        drop(handle);
    }
}
