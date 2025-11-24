//! Circuit Breaker for TCP Backends
//!
//! Implements the circuit breaker pattern to prevent cascading failures
//! when backends are unhealthy or unreachable.
//!
//! States:
//! - Closed: Normal operation, all requests pass through
//! - Open: Backend is failing, all requests fail fast
//! - Half-Open: Testing if backend has recovered
//!
//! Based on Netflix Hystrix and resilience4j patterns

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CircuitState {
    /// Circuit is closed, requests pass through
    Closed = 0,

    /// Circuit is open, requests fail fast
    Open = 1,

    /// Circuit is half-open, testing recovery
    HalfOpen = 2,
}

impl From<u8> for CircuitState {
    fn from(value: u8) -> Self {
        match value {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold before opening circuit
    pub failure_threshold: u32,

    /// Success threshold to close circuit from half-open
    pub success_threshold: u32,

    /// Time window for failure counting
    pub failure_window: Duration,

    /// Wait time before trying half-open from open
    pub wait_duration: Duration,

    /// Timeout for half-open test requests
    pub half_open_timeout: Duration,

    /// Maximum number of half-open requests
    pub half_open_max_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            failure_window: Duration::from_secs(10),
            wait_duration: Duration::from_secs(30),
            half_open_timeout: Duration::from_secs(5),
            half_open_max_requests: 3,
        }
    }
}

/// Circuit breaker for a single backend
pub struct CircuitBreaker {
    /// Backend address
    backend_addr: SocketAddr,

    /// Configuration
    config: CircuitBreakerConfig,

    /// Current state
    state: AtomicU8,

    /// State change details
    state_details: Arc<RwLock<StateDetails>>,

    /// Statistics
    stats: Arc<CircuitBreakerStats>,
}

/// State change details
#[derive(Debug, Clone)]
struct StateDetails {
    /// Time when state last changed
    last_state_change: Instant,

    /// Consecutive failures in current window
    consecutive_failures: u32,

    /// Consecutive successes in half-open state
    consecutive_successes: u32,

    /// Number of half-open requests currently in flight
    half_open_requests: u32,
}

impl StateDetails {
    fn new() -> Self {
        Self {
            last_state_change: Instant::now(),
            consecutive_failures: 0,
            consecutive_successes: 0,
            half_open_requests: 0,
        }
    }
}

/// Circuit breaker statistics
#[derive(Debug)]
pub struct CircuitBreakerStats {
    /// Total requests attempted
    pub total_requests: AtomicU64,

    /// Requests that succeeded
    pub successful_requests: AtomicU64,

    /// Requests that failed
    pub failed_requests: AtomicU64,

    /// Requests rejected (circuit open)
    pub rejected_requests: AtomicU64,

    /// Times circuit opened
    pub circuit_opened: AtomicU64,

    /// Times circuit closed
    pub circuit_closed: AtomicU64,

    /// Times circuit entered half-open
    pub circuit_half_opened: AtomicU64,
}

impl CircuitBreakerStats {
    fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            successful_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            rejected_requests: AtomicU64::new(0),
            circuit_opened: AtomicU64::new(0),
            circuit_closed: AtomicU64::new(0),
            circuit_half_opened: AtomicU64::new(0),
        }
    }

    /// Get snapshot of statistics
    pub fn snapshot(&self) -> CircuitBreakerStatsSnapshot {
        CircuitBreakerStatsSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            successful_requests: self.successful_requests.load(Ordering::Relaxed),
            failed_requests: self.failed_requests.load(Ordering::Relaxed),
            rejected_requests: self.rejected_requests.load(Ordering::Relaxed),
            circuit_opened: self.circuit_opened.load(Ordering::Relaxed),
            circuit_closed: self.circuit_closed.load(Ordering::Relaxed),
            circuit_half_opened: self.circuit_half_opened.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of circuit breaker statistics
#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerStatsSnapshot {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rejected_requests: u64,
    pub circuit_opened: u64,
    pub circuit_closed: u64,
    pub circuit_half_opened: u64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(backend_addr: SocketAddr, config: CircuitBreakerConfig) -> Self {
        info!(
            "Creating circuit breaker for {} (failure_threshold: {}, wait: {:?})",
            backend_addr, config.failure_threshold, config.wait_duration
        );

        Self {
            backend_addr,
            config,
            state: AtomicU8::new(CircuitState::Closed as u8),
            state_details: Arc::new(RwLock::new(StateDetails::new())),
            stats: Arc::new(CircuitBreakerStats::new()),
        }
    }

    /// Check if request is allowed
    pub async fn is_request_allowed(&self) -> bool {
        self.stats.total_requests.fetch_add(1, Ordering::Relaxed);

        let current_state = CircuitState::from(self.state.load(Ordering::Relaxed));

        match current_state {
            CircuitState::Closed => true,

            CircuitState::Open => {
                // Check if we should transition to half-open
                let details = self.state_details.read().await;
                if details.last_state_change.elapsed() >= self.config.wait_duration {
                    drop(details);
                    self.try_half_open().await;
                    true // Allow this request to test
                } else {
                    self.stats.rejected_requests.fetch_add(1, Ordering::Relaxed);
                    false
                }
            }

            CircuitState::HalfOpen => {
                // Check if we can allow more half-open requests
                let mut details = self.state_details.write().await;
                if details.half_open_requests < self.config.half_open_max_requests {
                    details.half_open_requests += 1;
                    true
                } else {
                    drop(details);
                    self.stats.rejected_requests.fetch_add(1, Ordering::Relaxed);
                    false
                }
            }
        }
    }

    /// Record successful request
    pub async fn record_success(&self) {
        self.stats.successful_requests.fetch_add(1, Ordering::Relaxed);

        let current_state = CircuitState::from(self.state.load(Ordering::Relaxed));

        match current_state {
            CircuitState::Closed => {
                // Reset failure count on success
                let mut details = self.state_details.write().await;
                details.consecutive_failures = 0;
            }

            CircuitState::HalfOpen => {
                let mut details = self.state_details.write().await;
                details.half_open_requests = details.half_open_requests.saturating_sub(1);
                details.consecutive_successes += 1;

                if details.consecutive_successes >= self.config.success_threshold {
                    drop(details);
                    self.close_circuit().await;
                }
            }

            CircuitState::Open => {
                // Shouldn't happen, but reset failures
                let mut details = self.state_details.write().await;
                details.consecutive_failures = 0;
            }
        }
    }

    /// Record failed request
    pub async fn record_failure(&self) {
        self.stats.failed_requests.fetch_add(1, Ordering::Relaxed);

        let current_state = CircuitState::from(self.state.load(Ordering::Relaxed));

        match current_state {
            CircuitState::Closed => {
                let mut details = self.state_details.write().await;
                details.consecutive_failures += 1;

                if details.consecutive_failures >= self.config.failure_threshold {
                    drop(details);
                    self.open_circuit().await;
                }
            }

            CircuitState::HalfOpen => {
                // Any failure in half-open immediately opens circuit
                let mut details = self.state_details.write().await;
                details.half_open_requests = details.half_open_requests.saturating_sub(1);
                drop(details);
                self.open_circuit().await;
            }

            CircuitState::Open => {
                // Already open, just update stats
            }
        }
    }

    /// Transition to open state
    async fn open_circuit(&self) {
        let old_state = CircuitState::from(
            self.state.swap(CircuitState::Open as u8, Ordering::SeqCst)
        );

        if old_state != CircuitState::Open {
            self.stats.circuit_opened.fetch_add(1, Ordering::Relaxed);

            let mut details = self.state_details.write().await;
            details.last_state_change = Instant::now();
            details.consecutive_failures = 0;
            details.consecutive_successes = 0;
            details.half_open_requests = 0;

            warn!(
                "Circuit breaker OPENED for {} (failures: {}, wait: {:?})",
                self.backend_addr,
                self.config.failure_threshold,
                self.config.wait_duration
            );
        }
    }

    /// Transition to half-open state
    async fn try_half_open(&self) {
        let old_state = CircuitState::from(
            self.state.swap(CircuitState::HalfOpen as u8, Ordering::SeqCst)
        );

        if old_state != CircuitState::HalfOpen {
            self.stats.circuit_half_opened.fetch_add(1, Ordering::Relaxed);

            let mut details = self.state_details.write().await;
            details.last_state_change = Instant::now();
            details.consecutive_successes = 0;
            details.half_open_requests = 0;

            info!(
                "Circuit breaker HALF-OPEN for {} (testing with {} requests)",
                self.backend_addr, self.config.half_open_max_requests
            );
        }
    }

    /// Transition to closed state
    async fn close_circuit(&self) {
        let old_state = CircuitState::from(
            self.state.swap(CircuitState::Closed as u8, Ordering::SeqCst)
        );

        if old_state != CircuitState::Closed {
            self.stats.circuit_closed.fetch_add(1, Ordering::Relaxed);

            let mut details = self.state_details.write().await;
            details.last_state_change = Instant::now();
            details.consecutive_failures = 0;
            details.consecutive_successes = 0;
            details.half_open_requests = 0;

            info!(
                "Circuit breaker CLOSED for {} (backend recovered)",
                self.backend_addr
            );
        }
    }

    /// Get current state
    pub fn get_state(&self) -> CircuitState {
        CircuitState::from(self.state.load(Ordering::Relaxed))
    }

    /// Get statistics
    pub fn stats(&self) -> CircuitBreakerStatsSnapshot {
        self.stats.snapshot()
    }

    /// Force open circuit (for testing or manual intervention)
    pub async fn force_open(&self) {
        self.open_circuit().await;
    }

    /// Force close circuit (for testing or manual intervention)
    pub async fn force_close(&self) {
        self.close_circuit().await;
    }

    /// Reset circuit breaker to initial state
    pub async fn reset(&self) {
        self.state.store(CircuitState::Closed as u8, Ordering::SeqCst);

        let mut details = self.state_details.write().await;
        details.last_state_change = Instant::now();
        details.consecutive_failures = 0;
        details.consecutive_successes = 0;
        details.half_open_requests = 0;

        debug!("Circuit breaker RESET for {}", self.backend_addr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };

        let cb = CircuitBreaker::new("127.0.0.1:8080".parse().unwrap(), config);

        assert_eq!(cb.get_state(), CircuitState::Closed);
        assert!(cb.is_request_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };

        let cb = CircuitBreaker::new("127.0.0.1:8080".parse().unwrap(), config);

        // Record 3 failures
        cb.record_failure().await;
        assert_eq!(cb.get_state(), CircuitState::Closed);

        cb.record_failure().await;
        assert_eq!(cb.get_state(), CircuitState::Closed);

        cb.record_failure().await;
        assert_eq!(cb.get_state(), CircuitState::Open);

        // Requests should be rejected
        assert!(!cb.is_request_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_half_open_transition() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            wait_duration: Duration::from_millis(100),
            ..Default::default()
        };

        let cb = CircuitBreaker::new("127.0.0.1:8080".parse().unwrap(), config);

        // Open circuit
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.get_state(), CircuitState::Open);

        // Wait for transition window
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Next request should trigger half-open
        assert!(cb.is_request_allowed().await);
        assert_eq!(cb.get_state(), CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn test_circuit_closes_on_success() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            wait_duration: Duration::from_millis(100),
            ..Default::default()
        };

        let cb = CircuitBreaker::new("127.0.0.1:8080".parse().unwrap(), config);

        // Open circuit
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.get_state(), CircuitState::Open);

        // Wait and transition to half-open
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(cb.is_request_allowed().await);

        // Record successes
        cb.record_success().await;
        assert_eq!(cb.get_state(), CircuitState::HalfOpen);

        cb.record_success().await;
        assert_eq!(cb.get_state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_half_open_failure_reopens() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            wait_duration: Duration::from_millis(100),
            ..Default::default()
        };

        let cb = CircuitBreaker::new("127.0.0.1:8080".parse().unwrap(), config);

        // Open circuit
        cb.record_failure().await;
        cb.record_failure().await;

        // Wait and go half-open
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(cb.is_request_allowed().await);
        assert_eq!(cb.get_state(), CircuitState::HalfOpen);

        // Any failure in half-open reopens circuit
        cb.record_failure().await;
        assert_eq!(cb.get_state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_stats() {
        let config = CircuitBreakerConfig::default();
        let cb = CircuitBreaker::new("127.0.0.1:8080".parse().unwrap(), config);

        cb.is_request_allowed().await;
        cb.record_success().await;

        cb.is_request_allowed().await;
        cb.record_failure().await;

        let stats = cb.stats();
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 1);
    }

    #[tokio::test]
    async fn test_force_open_close() {
        let config = CircuitBreakerConfig::default();
        let cb = CircuitBreaker::new("127.0.0.1:8080".parse().unwrap(), config);

        assert_eq!(cb.get_state(), CircuitState::Closed);

        cb.force_open().await;
        assert_eq!(cb.get_state(), CircuitState::Open);

        cb.force_close().await;
        assert_eq!(cb.get_state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };

        let cb = CircuitBreaker::new("127.0.0.1:8080".parse().unwrap(), config);

        // Open circuit
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.get_state(), CircuitState::Open);

        // Reset
        cb.reset().await;
        assert_eq!(cb.get_state(), CircuitState::Closed);
        assert!(cb.is_request_allowed().await);
    }
}
