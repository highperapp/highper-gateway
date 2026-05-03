//! Runtime module for io_uring-based async execution

mod worker;
mod buffer_pool;
mod signals;

// Week 2 optimization: Adapter pattern for pluggable I/O backends
mod io_backend;
mod epoll_backend;

// io_uring shim layer (Linux-only, feature-gated)
#[cfg(all(feature = "io-uring", target_os = "linux"))]
mod io_uring_shim;

#[cfg(all(feature = "io-uring", target_os = "linux"))]
mod io_uring_backend;

#[cfg(all(feature = "io-uring", target_os = "linux"))]
mod io_uring_buffers;

// Week 1, Day 1: Fixed borrow checker issues, now enabled
#[cfg(all(feature = "io-uring", target_os = "linux"))]
mod hybrid_stream;

// Week 9: Performance optimizations
// SIMD-accelerated operations for hot data processing paths
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
mod simd_opt;

// Week 10: SIMD helper functions for common use cases
// Higher-level wrappers around SIMD operations for parsing and validation
pub mod simd_helpers;

// Lock-free data structures for high-performance concurrent access
mod lockfree;

// CPU affinity and NUMA awareness for extreme scale
pub mod cpu_affinity;

// Backpressure and load shedding for graceful degradation
pub mod backpressure;

pub use worker::*;
pub use buffer_pool::*;
pub use signals::*;

// Export adapter pattern types
pub use io_backend::{AsyncIoBackend, BackendStats, GLOBAL_IO, is_io_uring_available};

#[cfg(all(feature = "io-uring", target_os = "linux"))]
pub use io_uring_shim::*;

#[cfg(all(feature = "io-uring", target_os = "linux"))]
pub use io_uring_buffers::{RegisteredBufferPool, RegisteredBuffer, REGISTERED_BUFFER_SIZE, NUM_REGISTERED_BUFFERS};

#[cfg(all(feature = "io-uring", target_os = "linux"))]
pub use hybrid_stream::HybridTcpStream;

// Export SIMD optimizations (only the beneficial ones - see WEEK10_BENCHMARK_RESULTS.md)
// NOTE: simd_memcpy and simd_memcmp are NOT exported as benchmarks showed they are 2-3x SLOWER than scalar
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub use simd_opt::{simd_find_pattern, simd_checksum};

// Export lock-free structures
pub use lockfree::{
    AtomicCounter, WorkStealingQueue, ConcurrentStats, StatsSnapshot, BoundedQueue
};

use crate::config::{Config, ConfigReloader};
use crate::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Main runtime for the proxy
pub struct Runtime {
    /// Shared configuration (can be updated via hot reload)
    config: Arc<RwLock<Config>>,

    /// Number of worker threads
    num_workers: usize,

    /// Configuration reloader (if hot reload is enabled)
    reloader: Option<ConfigReloader>,

    /// PID file for process tracking
    _pid_file: Option<PidFile>,
}

impl Runtime {
    /// Create a new runtime with the given configuration
    pub fn new(config: Config) -> Result<Self> {
        let num_workers = if config.server.workers == "auto" {
            num_cpus::get()
        } else {
            config.server.workers.parse()?
        };

        info!("Initializing runtime with {} workers", num_workers);

        // Create PID file for process tracking
        let pid_file = PidFile::create("/var/run/highper-gateway.pid")
            .or_else(|_| PidFile::create("/tmp/highper-gateway.pid"))
            .ok();

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            num_workers,
            reloader: None,
            _pid_file: pid_file,
        })
    }

    /// Create a new runtime with hot reload support
    pub fn with_hot_reload(config: Config, config_path: PathBuf) -> Result<Self> {
        let num_workers = if config.server.workers == "auto" {
            num_cpus::get()
        } else {
            config.server.workers.parse()?
        };

        info!("Initializing runtime with {} workers (hot reload enabled)", num_workers);

        // Create PID file for process tracking (supports reload command)
        let pid_file = PidFile::create("/var/run/highper-gateway.pid")
            .or_else(|_| PidFile::create("/tmp/highper-gateway.pid"))
            .ok();

        // Create reloader
        let mut reloader = ConfigReloader::new(&config_path, config)?;
        reloader.start_watching()?;

        // Get shared config from reloader
        let config = reloader.config();

        Ok(Self {
            config,
            num_workers,
            reloader: Some(reloader),
            _pid_file: pid_file,
        })
    }

    /// Run the proxy server
    pub async fn run(mut self) -> Result<()> {
        info!("Starting Highper Gateway server");

        // Read initial config
        let config_read = self.config.read().await;
        let metrics_enabled = config_read.observability.metrics.enabled;
        let metrics_bind = config_read.observability.metrics.bind.clone();
        drop(config_read);

        // Initialize metrics if enabled
        let metrics = if metrics_enabled {
            Some(Arc::new(crate::observability::Metrics::new()))
        } else {
            None
        };

        // Initialize compression system with all compression algorithms
        info!("Initializing compression system");
        crate::middleware::compression::init_compression();

        // Start the reloader task if hot reload is enabled
        let (reloader_task, reload_tx_for_admin) = if let Some(reloader) = self.reloader.take() {
            let reload_tx = reloader.reload_trigger();
            let reload_tx_for_admin = reload_tx.clone();

            // Spawn reloader task
            let reloader_handle = tokio::spawn(async move {
                if let Err(e) = reloader.run().await {
                    tracing::error!("Config reloader error: {}", e);
                }
            });

            // Set up signal handling with reload support
            let signal_handle = tokio::spawn(async move {
                setup_signals_with_reload(reload_tx).await;
            });

            (Some((reloader_handle, signal_handle)), Some(reload_tx_for_admin))
        } else {
            // No hot reload - use simple shutdown signal
            let signal_handle = tokio::spawn(async move {
                setup_shutdown_signal().await;
            });

            (Some((signal_handle, tokio::spawn(async {}))), None)
        };

        // Start the HTTP server
        let server_task = self.start_server();

        // Read config for TLS passthrough check
        let config_read = self.config.read().await;
        let tls_passthrough_enabled = config_read.tls.as_ref()
            .and_then(|tls| tls.passthrough.as_ref())
            .map(|pt| pt.enabled)
            .unwrap_or(false);
        drop(config_read);

        // Start TLS passthrough server if enabled
        let passthrough_task = if tls_passthrough_enabled {
            let config = self.config.clone();
            Some(tokio::spawn(async move {
                // Convert Arc<RwLock<Config>> to Arc<Config> for compatibility
                let cfg = config.read().await.clone();
                if let Err(e) = crate::proxy::Server::run_tls_passthrough(Arc::new(cfg)).await {
                    tracing::error!("TLS passthrough server error: {}", e);
                }
            }))
        } else {
            None
        };

        // Start observability server if metrics are enabled
        let observability_task = if let Some(metrics) = metrics {
            let bind_addr = metrics_bind.parse()?;
            let obs_server = crate::observability::ObservabilityServer::new(metrics, bind_addr);
            Some(tokio::spawn(async move {
                if let Err(e) = obs_server.run().await {
                    tracing::error!("Observability server error: {}", e);
                }
            }))
        } else {
            None
        };

        // Start admin API server if enabled
        let config_read = self.config.read().await;
        let admin_enabled = config_read.admin.as_ref().map(|a| a.enabled).unwrap_or(false);
        let admin_config = config_read.admin.clone();
        drop(config_read);

        let admin_task = if admin_enabled {
            if let Some(admin_cfg) = admin_config {
                let proxy_config = self.config.clone();
                let admin_server = if let Some(reload_tx) = reload_tx_for_admin {
                    crate::admin::AdminServer::with_reload_trigger(admin_cfg, proxy_config, reload_tx)
                } else {
                    crate::admin::AdminServer::new(admin_cfg, proxy_config)
                };
                Some(tokio::spawn(async move {
                    if let Err(e) = admin_server.run().await {
                        tracing::error!("Admin API server error: {}", e);
                    }
                }))
            } else {
                None
            }
        } else {
            None
        };

        // Start HTTP/3 server if enabled (using quiche implementation)
        let config_read = self.config.read().await;
        let http3_enabled = config_read.server.http3.enabled;
        drop(config_read);

        let http3_task = if http3_enabled {
            let proxy_config = self.config.clone();
            // Use quiche-based HTTP/3 implementation for superior performance
            let http3_server = crate::http::http3_quiche::Http3Server::new(proxy_config);
            Some(tokio::spawn(async move {
                if let Err(e) = http3_server.run().await {
                    tracing::error!("HTTP/3 server error: {}", e);
                }
            }))
        } else {
            None
        };

        // Wait for server error (signals are handled in background task)
        let result = server_task.await;
        result?;

        info!("Shutting down gracefully...");

        // B14: drain window for in-flight spawned tasks before forced abort.
        // Operators tune via HIGHPER_SHUTDOWN_SPAWN_TASK_DRAIN (default 10s)
        // per RuntimeConfig::shutdown.spawn_task_drain_secs. This is a
        // simple time-based drain — full per-task tracking is a future
        // refactor; for now a configurable delay is sufficient to let
        // long-running background work (e.g., cache writebacks, observability
        // flush, plugin teardown) complete before we abort.
        let drain = *crate::runtime_config::current().shutdown.spawn_task_drain_secs.get();
        if !drain.is_zero() {
            info!("Draining spawned tasks for {:?}", drain);
            tokio::time::sleep(drain).await;
        }

        // Cancel reloader task
        if let Some((reloader_handle, signal_handle)) = reloader_task {
            reloader_handle.abort();
            signal_handle.abort();
        }

        // Cancel TLS passthrough task
        if let Some(task) = passthrough_task {
            task.abort();
        }

        // Cancel observability task
        if let Some(task) = observability_task {
            task.abort();
        }

        // Cancel admin task
        if let Some(task) = admin_task {
            task.abort();
        }

        // Cancel HTTP/3 task
        if let Some(task) = http3_task {
            task.abort();
        }

        Ok(())
    }

    async fn start_server(&self) -> Result<()> {
        use crate::proxy::Server;

        // Read config and convert to Arc<Config> for compatibility
        let cfg = self.config.read().await.clone();
        let config = Arc::new(cfg);

        // Create and start the HTTP server
        let server = Server::new(config);
        server.run().await
    }
}

// Re-add num_cpus dependency
