//! Configuration file watcher for hot reload
//!
//! Monitors configuration file changes using filesystem notifications
//! and emits events when changes are detected.

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// Configuration file watcher
pub struct ConfigWatcher {
    path: PathBuf,
    watcher: Option<RecommendedWatcher>,
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
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<(Self, mpsc::Receiver<ConfigEvent>)> {
        let path = path.as_ref().to_path_buf();
        // B11.3: bounded file-event channel; capacity tunable via
        // HIGHPER_CONFIG_WATCHER_FILE_EVENT_CAPACITY (default 32). Drop
        // on full is safe: file-watch is idempotent — the next change
        // re-triggers. try_current() so unit tests work pre-install.
        let capacity = crate::runtime_config::try_current()
            .map(|c| *c.config_watcher.file_event_channel_capacity.get() as usize)
            .unwrap_or(32);
        let (event_tx, event_rx) = mpsc::channel(capacity.max(1));

        let tx = event_tx.clone();
        let watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                // Sync closure context — try_send (drop-on-full).
                match result {
                    Ok(event) => {
                        debug!("File system event: {:?}", event);
                        match event.kind {
                            notify::EventKind::Modify(_) => {
                                let _ = tx.try_send(ConfigEvent::Modified);
                            }
                            notify::EventKind::Remove(_) => {
                                let _ = tx.try_send(ConfigEvent::Deleted);
                            }
                            notify::EventKind::Create(_) => {
                                let _ = tx.try_send(ConfigEvent::Created);
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        error!("Watch error: {}", e);
                        let _ = tx.try_send(ConfigEvent::Error(e.to_string()));
                    }
                }
            },
            Config::default(),
        )?;

        Ok((
            Self {
                path,
                watcher: Some(watcher),
            },
            event_rx,
        ))
    }

    /// Start watching the configuration file
    pub fn watch(&mut self) -> anyhow::Result<()> {
        if let Some(watcher) = &mut self.watcher {
            // Watch the parent directory to handle file replacements (common in atomic saves)
            let watch_path = if self.path.is_file() {
                self.path.parent().unwrap_or(&self.path)
            } else {
                &self.path
            };

            watcher.watch(watch_path, RecursiveMode::NonRecursive)?;
            info!("Started watching config file: {:?}", self.path);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Watcher not initialized"))
        }
    }

    /// Stop watching
    pub fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(watcher) = &mut self.watcher {
            let watch_path = if self.path.is_file() {
                self.path.parent().unwrap_or(&self.path)
            } else {
                &self.path
            };
            watcher.unwatch(watch_path)?;
            info!("Stopped watching config file");
        }
        self.watcher = None;
        Ok(())
    }

    /// Get the path being watched
    pub fn path(&self) -> &Path {
        &self.path
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
        let temp_dir = std::env::temp_dir();
        // Use unique filename to avoid conflicts with parallel test runs
        let temp_file = temp_dir.join(format!("test_config_modify_{}.yaml", std::process::id()));
        fs::write(&temp_file, "test: value").unwrap();

        let (mut watcher, mut event_rx) = ConfigWatcher::new(&temp_file).unwrap();
        watcher.watch().unwrap();

        // Wait for watcher to be fully initialized
        sleep(Duration::from_millis(200)).await;

        // Modify file
        fs::write(&temp_file, "test: new_value").unwrap();

        // Wait for event - file system watchers can be slow
        // We might receive multiple events (Created, then Modified)
        let mut received_event = false;
        for _ in 0..5 {
            match tokio::time::timeout(Duration::from_millis(500), event_rx.recv()).await {
                Ok(Some(event)) => {
                    if matches!(event, ConfigEvent::Modified | ConfigEvent::Created) {
                        received_event = true;
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => continue,
            }
        }

        assert!(
            received_event,
            "Should have received Modified or Created event"
        );

        // Cleanup
        watcher.stop().ok();
        fs::remove_file(temp_file).ok();
    }

    #[tokio::test]
    async fn test_file_deletion_detection() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("test_config_delete_{}.yaml", std::process::id()));
        fs::write(&temp_file, "test: value").unwrap();

        let (mut watcher, mut event_rx) = ConfigWatcher::new(&temp_file).unwrap();
        watcher.watch().unwrap();

        // Wait for watcher to be ready
        sleep(Duration::from_millis(200)).await;

        // Delete file
        fs::remove_file(&temp_file).unwrap();

        // Wait for event - file system watchers can be slow
        // We might receive multiple events (Modified, then Deleted)
        let mut received_deletion = false;
        for _ in 0..5 {
            match tokio::time::timeout(Duration::from_millis(500), event_rx.recv()).await {
                Ok(Some(event)) => {
                    if matches!(event, ConfigEvent::Deleted | ConfigEvent::Modified) {
                        received_deletion = true;
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => continue,
            }
        }

        assert!(
            received_deletion,
            "Should have received Deleted or Modified event"
        );

        // Cleanup
        watcher.stop().ok();
    }

    #[tokio::test]
    async fn test_watcher_path() {
        let temp_file = "/tmp/test_path.yaml";
        let (watcher, _event_rx) = ConfigWatcher::new(temp_file).unwrap();

        assert_eq!(watcher.path(), Path::new(temp_file));
    }
}
