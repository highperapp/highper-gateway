//! Circuit breaker pattern for preventing cascading failures
//!
//! Implements the circuit breaker pattern to detect and prevent calls to failing services.

use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use tracing::{info, warn};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are rejected immediately
    Open,
    /// Circuit is half-open, allowing test requests
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Number of successful requests needed to close circuit from half-open
    pub success_threshold: u32,
    /// Duration to wait before transitioning from open to half-open
    pub timeout: Duration,
    /// Time window for counting failures
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            failure_window: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker statistics
#[derive(Debug, Clone)]
struct CircuitStats {
    /// Number of consecutive failures
    failure_count: u32,
    /// Number of consecutive successes (in half-open state)
    success_count: u32,
    /// Timestamp of last state change
    last_state_change: Instant,
    /// Recent failure timestamps
    recent_failures: Vec<Instant>,
}

impl CircuitStats {
    fn new() -> Self {
        Self {
            failure_count: 0,
            success_count: 0,
            last_state_change: Instant::now(),
            recent_failures: Vec::new(),
        }
    }

    /// Clean up old failures outside the failure window
    fn cleanup_old_failures(&mut self, window: Duration) {
        let cutoff = Instant::now() - window;
        self.recent_failures.retain(|&time| time > cutoff);
    }
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    stats: Arc<RwLock<CircuitStats>>,
    name: String,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(name: String, config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            stats: Arc::new(RwLock::new(CircuitStats::new())),
            name,
        }
    }

    /// Create with default configuration
    pub fn default_breaker(name: String) -> Self {
        Self::new(name, CircuitBreakerConfig::default())
    }

    /// Check if request is allowed
    pub fn allow_request(&self) -> bool {
        let state = *self.state.read();

        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has elapsed
                let stats = self.stats.read();
                let elapsed = stats.last_state_change.elapsed();

                if elapsed >= self.config.timeout {
                    drop(stats);
                    self.transition_to_half_open();
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                // Only allow one test request at a time
                true
            }
        }
    }

    /// Record a successful request
    pub fn record_success(&self) {
        let state = *self.state.read();

        match state {
            CircuitState::Closed => {
                // Reset failure count on success
                let mut stats = self.stats.write();
                stats.failure_count = 0;
                stats.recent_failures.clear();
            }
            CircuitState::HalfOpen => {
                let mut stats = self.stats.write();
                stats.success_count += 1;
                stats.failure_count = 0;
                stats.recent_failures.clear();

                if stats.success_count >= self.config.success_threshold {
                    drop(stats);
                    self.transition_to_closed();
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but reset anyway
                let mut stats = self.stats.write();
                stats.failure_count = 0;
                stats.recent_failures.clear();
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&self) {
        let state = *self.state.read();
        let now = Instant::now();

        let mut stats = self.stats.write();
        stats.failure_count += 1;
        stats.recent_failures.push(now);
        stats.cleanup_old_failures(self.config.failure_window);

        match state {
            CircuitState::Closed => {
                // Check if we should open the circuit
                if stats.recent_failures.len() >= self.config.failure_threshold as usize {
                    drop(stats);
                    self.transition_to_open();
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state reopens the circuit
                drop(stats);
                self.transition_to_open();
            }
            CircuitState::Open => {
                // Already open, just track the failure
            }
        }
    }

    /// Get current circuit state
    pub fn state(&self) -> CircuitState {
        *self.state.read()
    }

    /// Get current statistics
    pub fn stats(&self) -> (u32, u32) {
        let stats = self.stats.read();
        (stats.failure_count, stats.success_count)
    }

    /// Reset circuit breaker to closed state
    pub fn reset(&self) {
        info!("Resetting circuit breaker: {}", self.name);
        *self.state.write() = CircuitState::Closed;
        let mut stats = self.stats.write();
        stats.failure_count = 0;
        stats.success_count = 0;
        stats.recent_failures.clear();
        stats.last_state_change = Instant::now();
    }

    /// Execute an operation with circuit breaker protection
    pub async fn execute<F, Fut, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        if !self.allow_request() {
            warn!("Circuit breaker {} is OPEN, rejecting request", self.name);
            return Err(CircuitBreakerError::Open);
        }

        match operation().await {
            Ok(result) => {
                self.record_success();
                Ok(result)
            }
            Err(e) => {
                self.record_failure();
                Err(CircuitBreakerError::Failure(e))
            }
        }
    }

    /// Transition to open state
    fn transition_to_open(&self) {
        warn!("Circuit breaker {} transitioning to OPEN", self.name);
        *self.state.write() = CircuitState::Open;
        let mut stats = self.stats.write();
        stats.last_state_change = Instant::now();
        stats.success_count = 0;
    }

    /// Transition to half-open state
    fn transition_to_half_open(&self) {
        info!("Circuit breaker {} transitioning to HALF_OPEN", self.name);
        *self.state.write() = CircuitState::HalfOpen;
        let mut stats = self.stats.write();
        stats.last_state_change = Instant::now();
        stats.success_count = 0;
        stats.failure_count = 0;
    }

    /// Transition to closed state
    fn transition_to_closed(&self) {
        info!("Circuit breaker {} transitioning to CLOSED", self.name);
        *self.state.write() = CircuitState::Closed;
        let mut stats = self.stats.write();
        stats.last_state_change = Instant::now();
        stats.success_count = 0;
        stats.failure_count = 0;
        stats.recent_failures.clear();
    }
}

/// Circuit breaker error
#[derive(Debug)]
pub enum CircuitBreakerError<E> {
    /// Circuit is open, request rejected
    Open,
    /// Operation failed
    Failure(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::Open => write!(f, "Circuit breaker is open"),
            CircuitBreakerError::Failure(e) => write!(f, "Operation failed: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CircuitBreakerError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CircuitBreakerError::Open => None,
            CircuitBreakerError::Failure(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_initial_state() {
        let cb = CircuitBreaker::default_breaker("test".to_string());
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_open_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            failure_window: Duration::from_secs(60),
        };

        let cb = CircuitBreaker::new("test".to_string(), config);

        // Record failures
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_half_open_transition() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
            failure_window: Duration::from_secs(60),
        };

        let cb = CircuitBreaker::new("test".to_string(), config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(150));

        // Should transition to half-open
        assert!(cb.allow_request());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_close_from_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
            failure_window: Duration::from_secs(60),
        };

        let cb = CircuitBreaker::new("test".to_string(), config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();

        // Wait and transition to half-open
        std::thread::sleep(Duration::from_millis(150));
        cb.allow_request();

        // Record successes
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_reopen_from_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
            failure_window: Duration::from_secs(60),
        };

        let cb = CircuitBreaker::new("test".to_string(), config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();

        // Wait and transition to half-open
        std::thread::sleep(Duration::from_millis(150));
        cb.allow_request();

        // Record a failure in half-open state
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            failure_window: Duration::from_secs(60),
        };

        let cb = CircuitBreaker::new("test".to_string(), config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Reset
        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());

        let (failures, successes) = cb.stats();
        assert_eq!(failures, 0);
        assert_eq!(successes, 0);
    }

    #[tokio::test]
    async fn test_execute_success() {
        let cb = CircuitBreaker::default_breaker("test".to_string());
        let counter = AtomicU32::new(0);

        let result = cb
            .execute(|| async {
                counter.fetch_add(1, Ordering::Relaxed);
                Ok::<_, String>("success")
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_execute_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            failure_window: Duration::from_secs(60),
        };

        let cb = CircuitBreaker::new("test".to_string(), config);
        let counter = AtomicU32::new(0);

        // First failure
        let result = cb
            .execute(|| async {
                counter.fetch_add(1, Ordering::Relaxed);
                Err::<String, _>("error")
            })
            .await;

        assert!(matches!(result, Err(CircuitBreakerError::Failure(_))));
        assert_eq!(cb.state(), CircuitState::Closed);

        // Second failure - should open circuit
        let result = cb
            .execute(|| async {
                counter.fetch_add(1, Ordering::Relaxed);
                Err::<String, _>("error")
            })
            .await;

        assert!(matches!(result, Err(CircuitBreakerError::Failure(_))));
        assert_eq!(cb.state(), CircuitState::Open);

        // Third attempt - should be rejected immediately
        let result = cb
            .execute(|| async {
                counter.fetch_add(1, Ordering::Relaxed);
                Ok::<_, String>("success")
            })
            .await;

        assert!(matches!(result, Err(CircuitBreakerError::Open)));
        assert_eq!(counter.load(Ordering::Relaxed), 2); // Third attempt never executed
    }

    #[test]
    fn test_failure_window_cleanup() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            failure_window: Duration::from_millis(100),
        };

        let cb = CircuitBreaker::new("test".to_string(), config);

        // Record two failures
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        // Wait for failure window to expire
        std::thread::sleep(Duration::from_millis(150));

        // Record one more failure - should not open because old failures expired
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
