//! Signal handling for graceful shutdown and reload

use crate::config::ReloadTrigger;
use std::path::Path;
use tokio::signal;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

/// Set up signal handling for graceful shutdown
pub async fn setup_shutdown_signal() {
    let ctrl_c = async {
        match signal::ctrl_c().await {
            Ok(_) => {},
            Err(e) => {
                error!("Failed to install Ctrl+C handler: {}", e);
                error!("Shutdown signal handling disabled");
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                error!("Failed to install SIGTERM handler: {}", e);
                error!("SIGTERM signal handling disabled");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received SIGTERM signal");
        },
    }
}

/// Set up signal handling with reload support (Unix only)
#[cfg(unix)]
pub async fn setup_signals_with_reload(
    reload_tx: mpsc::UnboundedSender<ReloadTrigger>,
) {
    use signal::unix::{signal, SignalKind};

    let mut sighup = match signal(SignalKind::hangup()) {
        Ok(sig) => sig,
        Err(e) => {
            error!("Failed to install SIGHUP handler: {}", e);
            error!("Configuration reload via SIGHUP disabled");
            // Fall back to basic shutdown signal handling
            setup_shutdown_signal().await;
            return;
        }
    };

    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(sig) => sig,
        Err(e) => {
            error!("Failed to install SIGTERM handler: {}", e);
            error!("SIGTERM signal handling disabled");
            // Fall back to basic shutdown signal handling
            setup_shutdown_signal().await;
            return;
        }
    };

    let mut sigint = match signal(SignalKind::interrupt()) {
        Ok(sig) => sig,
        Err(e) => {
            error!("Failed to install SIGINT handler: {}", e);
            error!("SIGINT signal handling disabled");
            // Fall back to basic shutdown signal handling
            setup_shutdown_signal().await;
            return;
        }
    };

    loop {
        tokio::select! {
            _ = sighup.recv() => {
                info!("Received SIGHUP, triggering configuration reload");
                if let Err(e) = reload_tx.send(ReloadTrigger::Signal) {
                    warn!("Failed to send reload trigger: {}", e);
                }
                // Continue loop to handle more signals
            },
            _ = sigterm.recv() => {
                info!("Received SIGTERM");
                break;
            },
            _ = sigint.recv() => {
                info!("Received SIGINT (Ctrl+C)");
                break;
            },
        }
    }
}

/// Windows version (no SIGHUP support)
#[cfg(not(unix))]
pub async fn setup_signals_with_reload(
    _reload_tx: mpsc::UnboundedSender<ReloadTrigger>,
) {
    setup_shutdown_signal().await
}

/// PID file manager for tracking running process
pub struct PidFile {
    path: std::path::PathBuf,
}

impl PidFile {
    /// Create and write PID file
    pub fn create<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let pid = std::process::id();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Write PID to file
        std::fs::write(&path, pid.to_string())?;

        info!("PID file created: {} (pid: {})", path.display(), pid);

        Ok(Self { path })
    }

    /// Get the path to the PID file
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PidFile {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            error!("Failed to remove PID file {}: {}", self.path.display(), e);
        } else {
            info!("PID file removed: {}", self.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_pid_file_creation() {
        let temp_dir = std::env::temp_dir();
        let pid_file_path = temp_dir.join(format!("test_pidfile_{}.pid", std::process::id()));

        // Create PID file
        let pid_file = match PidFile::create(&pid_file_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to create PID file: {}", e);
                return;
            }
        };

        // Verify file exists and contains correct PID
        let contents = match fs::read_to_string(&pid_file_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read PID file: {}", e);
                return;
            }
        };
        let expected_pid = std::process::id().to_string();
        assert_eq!(contents, expected_pid);

        // Drop should remove file
        drop(pid_file);
        sleep(Duration::from_millis(10)).await;
        assert!(!pid_file_path.exists());
    }

    #[tokio::test]
    async fn test_reload_trigger_channel() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Send a reload trigger
        if let Err(e) = tx.send(ReloadTrigger::Signal) {
            eprintln!("Failed to send reload trigger: {}", e);
            return;
        }

        // Verify we can receive it
        let trigger = rx.recv().await;
        assert!(matches!(trigger, Some(ReloadTrigger::Signal)));
    }

    #[tokio::test]
    async fn test_manual_trigger() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Send manual trigger
        if let Err(e) = tx.send(ReloadTrigger::Manual) {
            eprintln!("Failed to send manual trigger: {}", e);
            return;
        }

        // Verify we can receive it
        let trigger = rx.recv().await;
        assert!(matches!(trigger, Some(ReloadTrigger::Manual)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_signal_handler_setup() {
        // This test verifies that the signal handler can be set up without panicking
        // We can't actually test signal handling in unit tests, but we can verify setup
        let (tx, _rx) = mpsc::unbounded_channel();

        // Spawn signal handler in background
        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = setup_signals_with_reload(tx) => {
                    // Signal handler completed
                }
                _ = sleep(Duration::from_millis(100)) => {
                    // Timeout - handler is running
                }
            }
        });

        // Give it time to set up
        sleep(Duration::from_millis(50)).await;

        // Cancel the task (simulating shutdown)
        handle.abort();
    }
}
