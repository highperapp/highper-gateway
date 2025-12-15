//! TCP Health Checker
//!
//! Protocol-aware health checks for MySQL, PostgreSQL, and Redis

use super::protocol::{MysqlProtocol, PostgresqlProtocol, RedisProtocol};
use super::{HealthCheckType, TcpBackend, TcpHealthCheckConfig};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::time::{interval, timeout};
use tracing::{debug, info, warn};

/// Health status of a backend
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Backend is healthy
    Healthy,

    /// Backend is unhealthy
    Unhealthy,

    /// Health status unknown (not yet checked)
    Unknown,
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Backend address
    pub addr: SocketAddr,

    /// Health status
    pub status: HealthStatus,

    /// Last check time
    pub last_check: std::time::Instant,

    /// Consecutive successes
    pub consecutive_successes: u32,

    /// Consecutive failures
    pub consecutive_failures: u32,

    /// Last error message
    pub last_error: Option<String>,
}

/// TCP Health Checker
pub struct TcpHealthChecker {
    backends: Vec<TcpBackend>,
    config: TcpHealthCheckConfig,
    results: Arc<RwLock<HashMap<SocketAddr, HealthCheckResult>>>,
    shutdown_tx: Option<tokio::sync::broadcast::Sender<()>>,
}

impl TcpHealthChecker {
    /// Create a new health checker
    pub fn new(backends: Vec<TcpBackend>, config: TcpHealthCheckConfig) -> Self {
        // Initialize results
        let mut results_map = HashMap::new();
        for backend in &backends {
            results_map.insert(
                backend.addr,
                HealthCheckResult {
                    addr: backend.addr,
                    status: HealthStatus::Unknown,
                    last_check: std::time::Instant::now(),
                    consecutive_successes: 0,
                    consecutive_failures: 0,
                    last_error: None,
                },
            );
        }

        let results = Arc::new(RwLock::new(results_map));

        Self {
            backends,
            config,
            results,
            shutdown_tx: None,
        }
    }

    /// Start health checking loop
    pub async fn start(&mut self) -> Result<()> {
        info!(
            "Starting health checker for {} backends (interval: {:?})",
            self.backends.len(),
            self.config.interval
        );

        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        let mut shutdown_rx = shutdown_tx.subscribe();
        let mut check_interval = interval(self.config.interval);

        loop {
            tokio::select! {
                _ = check_interval.tick() => {
                    self.check_all_backends().await;
                }

                _ = shutdown_rx.recv() => {
                    info!("Health checker shutdown signal received");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Check all backends
    async fn check_all_backends(&self) {
        debug!("Running health checks for {} backends", self.backends.len());

        let mut tasks = Vec::new();

        for backend in &self.backends {
            let backend = backend.clone();
            let config = backend.health_check.clone().unwrap_or(self.config.clone());
            let results = self.results.clone();

            let task = tokio::spawn(async move {
                let check_result = Self::check_backend(&backend, &config).await;

                // Update results
                let mut results_lock = results.write().await;
                if let Some(result) = results_lock.get_mut(&backend.addr) {
                    result.last_check = std::time::Instant::now();

                    match check_result {
                        Ok(_) => {
                            result.consecutive_successes += 1;
                            result.consecutive_failures = 0;
                            result.last_error = None;

                            // Mark as healthy if threshold met
                            if result.consecutive_successes >= config.healthy_threshold {
                                if result.status != HealthStatus::Healthy {
                                    info!("Backend {} is now healthy", backend.addr);
                                    result.status = HealthStatus::Healthy;
                                }
                            }
                        }
                        Err(e) => {
                            result.consecutive_failures += 1;
                            result.consecutive_successes = 0;
                            result.last_error = Some(e.to_string());

                            // Mark as unhealthy if threshold met
                            if result.consecutive_failures >= config.unhealthy_threshold {
                                if result.status != HealthStatus::Unhealthy {
                                    warn!("Backend {} is now unhealthy: {}", backend.addr, e);
                                    result.status = HealthStatus::Unhealthy;
                                }
                            }
                        }
                    }
                }
            });

            tasks.push(task);
        }

        // Wait for all health checks to complete
        for task in tasks {
            let _ = task.await;
        }
    }

    /// Check a single backend
    async fn check_backend(backend: &TcpBackend, config: &TcpHealthCheckConfig) -> Result<()> {
        let check_timeout = config.timeout;

        timeout(check_timeout, async {
            match config.check_type {
                HealthCheckType::Tcp => Self::check_tcp(backend.addr).await,
                HealthCheckType::Mysql => Self::check_mysql(backend.addr).await,
                HealthCheckType::Postgresql => Self::check_postgresql(backend.addr).await,
                HealthCheckType::Redis => Self::check_redis(backend.addr).await,
                HealthCheckType::Custom => Self::check_tcp(backend.addr).await, // Fallback
            }
        })
        .await
        .context("Health check timeout")?
    }

    /// TCP connection health check
    async fn check_tcp(addr: SocketAddr) -> Result<()> {
        let _stream = TcpStream::connect(addr)
            .await
            .context("TCP connection failed")?;

        Ok(())
    }

    /// MySQL protocol health check
    async fn check_mysql(addr: SocketAddr) -> Result<()> {
        let mut stream = TcpStream::connect(addr)
            .await
            .context("MySQL connection failed")?;

        // Read MySQL handshake
        let mut buf = vec![0u8; 1024];
        let n = stream.read(&mut buf).await.context("Failed to read handshake")?;

        if n < 5 {
            return Err(anyhow::anyhow!("Invalid MySQL handshake"));
        }

        // Send PING packet
        let ping = MysqlProtocol::ping_packet();
        stream.write_all(&ping).await.context("Failed to send PING")?;

        // Read response
        let n = stream.read(&mut buf).await.context("Failed to read PING response")?;

        if n == 0 {
            return Err(anyhow::anyhow!("Empty PING response"));
        }

        // Check for error packet
        if MysqlProtocol::is_error_packet(&buf[..n]) {
            return Err(anyhow::anyhow!("MySQL error response"));
        }

        Ok(())
    }

    /// PostgreSQL protocol health check
    async fn check_postgresql(addr: SocketAddr) -> Result<()> {
        let mut stream = TcpStream::connect(addr)
            .await
            .context("PostgreSQL connection failed")?;

        // Send simple query: SELECT 1
        let query = PostgresqlProtocol::simple_query("SELECT 1");
        stream.write_all(&query).await.context("Failed to send query")?;

        // Read response
        let mut buf = vec![0u8; 1024];
        let n = stream.read(&mut buf).await.context("Failed to read response")?;

        if n == 0 {
            return Err(anyhow::anyhow!("Empty query response"));
        }

        // Check for error message
        if PostgresqlProtocol::is_error_message(&buf[..n]) {
            return Err(anyhow::anyhow!("PostgreSQL error response"));
        }

        Ok(())
    }

    /// Redis protocol health check
    async fn check_redis(addr: SocketAddr) -> Result<()> {
        let mut stream = TcpStream::connect(addr)
            .await
            .context("Redis connection failed")?;

        // Send PING command
        let ping = RedisProtocol::ping_command();
        stream.write_all(&ping).await.context("Failed to send PING")?;

        // Read response
        let mut buf = vec![0u8; 1024];
        let n = stream.read(&mut buf).await.context("Failed to read response")?;

        if n == 0 {
            return Err(anyhow::anyhow!("Empty PING response"));
        }

        // Check for +PONG or error
        if RedisProtocol::is_error_message(&buf[..n]) {
            return Err(anyhow::anyhow!("Redis error response"));
        }

        // Expect "+PONG\r\n"
        if !buf[..n].starts_with(b"+PONG") && !buf[..n].starts_with(b"+OK") {
            return Err(anyhow::anyhow!("Unexpected Redis response"));
        }

        Ok(())
    }

    /// Get health status for a backend
    pub async fn get_health(&self, addr: &SocketAddr) -> Option<HealthStatus> {
        let results = self.results.read().await;
        results.get(addr).map(|r| r.status)
    }

    /// Get all health check results
    pub async fn get_all_results(&self) -> Vec<HealthCheckResult> {
        let results = self.results.read().await;
        results.values().cloned().collect()
    }

    /// Get healthy backends
    pub async fn get_healthy_backends(&self) -> Vec<SocketAddr> {
        let results = self.results.read().await;
        results
            .values()
            .filter(|r| r.status == HealthStatus::Healthy)
            .map(|r| r.addr)
            .collect()
    }

    /// Shutdown health checker
    pub async fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(());
            info!("Health checker shutdown initiated");
        }
    }

    /// Spawn health checker in background
    pub fn spawn(mut self) -> tokio::task::JoinHandle<Result<()>> {
        tokio::spawn(async move { self.start().await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_health_status() {
        let status = HealthStatus::Healthy;
        assert_eq!(status, HealthStatus::Healthy);
        assert_ne!(status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_check_result() {
        let result = HealthCheckResult {
            addr: "127.0.0.1:3306".parse().expect("Valid test address"),
            status: HealthStatus::Healthy,
            last_check: std::time::Instant::now(),
            consecutive_successes: 3,
            consecutive_failures: 0,
            last_error: None,
        };

        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(result.consecutive_successes, 3);
    }

    #[tokio::test]
    async fn test_health_checker_creation() {
        let backends = vec![TcpBackend {
            addr: "127.0.0.1:3306".parse().expect("Valid test address"),
            weight: 1,
            max_conns: None,
            health_check: None,
        }];

        let config = TcpHealthCheckConfig {
            check_type: HealthCheckType::Tcp,
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            healthy_threshold: 2,
            unhealthy_threshold: 2,
        };

        let checker = TcpHealthChecker::new(backends, config);
        assert_eq!(checker.backends.len(), 1);
    }

    #[tokio::test]
    async fn test_get_all_results() {
        let backends = vec![
            TcpBackend {
                addr: "127.0.0.1:3306".parse().expect("Valid test address"),
                weight: 1,
                max_conns: None,
                health_check: None,
            },
            TcpBackend {
                addr: "127.0.0.1:3307".parse().expect("Valid test address"),
                weight: 1,
                max_conns: None,
                health_check: None,
            },
        ];

        let config = TcpHealthCheckConfig {
            check_type: HealthCheckType::Tcp,
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            healthy_threshold: 2,
            unhealthy_threshold: 2,
        };

        let checker = TcpHealthChecker::new(backends, config);
        let results = checker.get_all_results().await;

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.status == HealthStatus::Unknown));
    }
}
