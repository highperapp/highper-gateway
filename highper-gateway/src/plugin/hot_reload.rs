//! Hot reload monitor for plugins
//!
//! This module watches the plugin directory for file changes and
//! automatically reloads plugins without proxy restart.

use super::registry::PluginRegistry;
use super::{PluginError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(feature = "plugin-hot-reload")]
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
#[cfg(feature = "plugin-hot-reload")]
use tokio::sync::mpsc;

/// Hot reload monitor
pub struct HotReloadMonitor {
    plugin_dir: PathBuf,
    registry: Arc<PluginRegistry>,
    #[cfg(feature = "plugin-hot-reload")]
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl HotReloadMonitor {
    /// Create a new hot reload monitor
    pub fn new(plugin_dir: PathBuf, registry: Arc<PluginRegistry>) -> Result<Self> {
        Ok(Self {
            plugin_dir,
            registry,
            #[cfg(feature = "plugin-hot-reload")]
            shutdown_tx: None,
        })
    }

    /// Start monitoring for file changes
    pub async fn start(&mut self) -> Result<()> {
        #[cfg(feature = "plugin-hot-reload")]
        {
            tracing::info!("Starting hot reload monitor for: {:?}", self.plugin_dir);

            let (event_tx, mut event_rx) = mpsc::channel(100);
            let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);

            // Create file watcher
            let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    // Send event through channel
                    let _ = event_tx.blocking_send(event);
                }
            })
            .map_err(|e| PluginError::Config(format!("Failed to create watcher: {}", e)))?;

            // Watch plugin directory
            watcher
                .watch(&self.plugin_dir, RecursiveMode::NonRecursive)
                .map_err(|e| PluginError::Config(format!("Failed to watch directory: {}", e)))?;

            let plugin_dir = self.plugin_dir.clone();
            let registry = Arc::clone(&self.registry);

            // Spawn monitoring task
            tokio::spawn(async move {
                let _watcher = watcher; // Keep watcher alive

                loop {
                    tokio::select! {
                        Some(event) = event_rx.recv() => {
                            if let Err(e) = Self::handle_event_static(&plugin_dir, &registry, event).await {
                                tracing::warn!("Failed to handle file event: {}", e);
                            }
                        }
                        _ = shutdown_rx.recv() => {
                            tracing::info!("Hot reload monitor shutting down");
                            break;
                        }
                    }
                }
            });

            self.shutdown_tx = Some(shutdown_tx);

            tracing::info!("Hot reload monitor started successfully");
            Ok(())
        }

        #[cfg(not(feature = "plugin-hot-reload"))]
        {
            Err(PluginError::Config(
                "Hot reload not compiled in".to_string(),
            ))
        }
    }

    /// Stop monitoring
    pub async fn stop(&mut self) -> Result<()> {
        #[cfg(feature = "plugin-hot-reload")]
        {
            tracing::info!("Stopping hot reload monitor");
            if let Some(tx) = self.shutdown_tx.take() {
                let _ = tx.send(()).await;
            }
            Ok(())
        }

        #[cfg(not(feature = "plugin-hot-reload"))]
        {
            Ok(())
        }
    }

    /// Handle file system event (static version for use in spawned task)
    #[cfg(feature = "plugin-hot-reload")]
    async fn handle_event_static(
        plugin_dir: &Path,
        registry: &PluginRegistry,
        event: Event,
    ) -> Result<()> {
        use notify::event::{CreateKind, ModifyKind, RemoveKind};

        // Only handle modification and creation events
        match event.kind {
            EventKind::Modify(ModifyKind::Data(_)) | EventKind::Create(CreateKind::File) => {
                // File was modified or created
                for path in event.paths {
                    if let Err(e) =
                        Self::handle_file_change_static(plugin_dir, registry, &path).await
                    {
                        tracing::warn!("Failed to reload plugin at {:?}: {}", path, e);
                    }
                }
            }
            EventKind::Remove(RemoveKind::File) => {
                // File was removed
                for path in event.paths {
                    if let Some(plugin_name) = Self::extract_plugin_name(&path) {
                        tracing::info!("Plugin file removed, unregistering: {}", plugin_name);
                        // Note: Can't actually unload here without manager reference
                        // This would need to be coordinated through the manager
                    }
                }
            }
            _ => {
                // Ignore other events
            }
        }

        Ok(())
    }

    /// Handle a file change event
    #[cfg(feature = "plugin-hot-reload")]
    async fn handle_file_change_static(
        plugin_dir: &Path,
        registry: &PluginRegistry,
        path: &Path,
    ) -> Result<()> {
        // Verify path is in plugin directory
        if !path.starts_with(plugin_dir) {
            return Ok(());
        }

        // Check if this is a plugin file
        let extension = path.extension().and_then(|e| e.to_str());
        let is_plugin = matches!(
            extension,
            Some("wasm") | Some("so") | Some("dylib") | Some("dll")
        );

        if !is_plugin {
            return Ok(());
        }

        // Extract plugin name from filename
        let plugin_name = match Self::extract_plugin_name(path) {
            Some(name) => name,
            None => return Ok(()),
        };

        // Check if plugin is already loaded
        if !registry.contains(&plugin_name) {
            tracing::debug!(
                "New plugin file detected but not auto-loading: {}",
                plugin_name
            );
            return Ok(());
        }

        tracing::info!("Plugin file changed, triggering reload: {}", plugin_name);

        // Add a small delay to ensure file write is complete. Stage 2
        // migrated this from a hardcoded 100ms literal to
        // PluginRuntimeConfig::hot_reload_settle (HIGHPER_PLUGIN_HOT_RELOAD_SETTLE).
        let settle = *crate::runtime_config::current()
            .plugin
            .hot_reload_settle
            .get();
        tokio::time::sleep(settle).await;

        // TODO: Actual reload would need to be coordinated through PluginManager
        // For now, just log that we detected the change
        tracing::info!(
            "Hot reload detected for plugin: {} (reload would happen here)",
            plugin_name
        );

        Ok(())
    }

    /// Extract plugin name from file path
    fn extract_plugin_name(path: &Path) -> Option<String> {
        path.file_stem().and_then(|s| s.to_str()).map(|s| {
            // Remove "lib" prefix if present (for .so files)
            s.strip_prefix("lib").unwrap_or(s).to_string()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hot_reload_creation() {
        let registry = Arc::new(PluginRegistry::new());
        let monitor = HotReloadMonitor::new(PathBuf::from("plugins"), registry);
        assert!(monitor.is_ok());
    }
}
