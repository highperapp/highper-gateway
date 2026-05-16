//! WebSocket Error Recovery and Reconnection Logic
//!
//! Provides automatic error detection, classification, and recovery mechanisms
//! for WebSocket connections. Includes retry logic with exponential backoff
//! and circuit breaker pattern for failing backends.

use crate::websocket::{ConnectionId, ConnectionTracker};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Error recovery configuration
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    /// Enable error recovery
    pub enabled: bool,

    /// Maximum number of retry attempts
    pub max_retries: u32,

    /// Initial retry delay
    pub initial_retry_delay: Duration,

    /// Maximum retry delay (for exponential backoff)
    pub max_retry_delay: Duration,

    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,

    /// Circuit breaker failure threshold
    pub circuit_breaker_threshold: u32,

    /// Circuit breaker timeout (how long to wait before trying again)
    pub circuit_breaker_timeout: Duration,

    /// Enable automatic reconnection
    pub auto_reconnect: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 3,
            initial_retry_delay: Duration::from_millis(100),
            max_retry_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout: Duration::from_secs(60),
            auto_reconnect: true,
        }
    }
}

/// WebSocket error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSocketError {
    /// Connection refused (backend down)
    ConnectionRefused,

    /// Connection timeout
    Timeout,

    /// Connection reset by peer
    ConnectionReset,

    /// Protocol error (invalid WebSocket frames)
    ProtocolError,

    /// Backend error (5xx response)
    BackendError,

    /// Client error (4xx response)
    ClientError,

    /// Network error (DNS, routing, etc.)
    NetworkError,

    /// Unknown error
    Unknown,
}

impl WebSocketError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            WebSocketError::ConnectionRefused => true,
            WebSocketError::Timeout => true,
            WebSocketError::ConnectionReset => true,
            WebSocketError::NetworkError => true,
            WebSocketError::BackendError => true,
            WebSocketError::ProtocolError => false,
            WebSocketError::ClientError => false,
            WebSocketError::Unknown => false,
        }
    }

    /// Get error severity (1 = low, 5 = critical)
    pub fn severity(&self) -> u8 {
        match self {
            WebSocketError::ConnectionRefused => 4,
            WebSocketError::Timeout => 3,
            WebSocketError::ConnectionReset => 3,
            WebSocketError::NetworkError => 4,
            WebSocketError::BackendError => 4,
            WebSocketError::ProtocolError => 5,
            WebSocketError::ClientError => 2,
            WebSocketError::Unknown => 3,
        }
    }
}

/// Circuit breaker state for a backend
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed (normal operation)
    Closed,

    /// Circuit is open (failing, reject requests)
    Open,

    /// Circuit is half-open (testing recovery)
    HalfOpen,
}

/// Circuit breaker for a backend
struct CircuitBreaker {
    state: Arc<std::sync::RwLock<CircuitState>>,
    failure_count: AtomicU32,
    last_failure_time: Arc<std::sync::RwLock<Option<Instant>>>,
    config: RecoveryConfig,
}

impl CircuitBreaker {
    fn new(config: RecoveryConfig) -> Self {
        Self {
            state: Arc::new(std::sync::RwLock::new(CircuitState::Closed)),
            failure_count: AtomicU32::new(0),
            last_failure_time: Arc::new(std::sync::RwLock::new(None)),
            config,
        }
    }

    /// Record a successful operation
    fn record_success(&self) {
        let mut state = self.state.write().unwrap();

        match *state {
            CircuitState::Open => {
                info!("Circuit breaker transitioning from Open to Closed (successful request)");
                *state = CircuitState::Closed;
                self.failure_count.store(0, Ordering::Relaxed);
                *self.last_failure_time.write().unwrap() = None;
            }
            CircuitState::HalfOpen => {
                info!("Circuit breaker transitioning from HalfOpen to Closed (successful request)");
                *state = CircuitState::Closed;
                self.failure_count.store(0, Ordering::Relaxed);
                *self.last_failure_time.write().unwrap() = None;
            }
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
            }
        }
    }

    /// Record a failure
    fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        *self.last_failure_time.write().unwrap() = Some(Instant::now());

        if failures >= self.config.circuit_breaker_threshold {
            let mut state = self.state.write().unwrap();
            if *state == CircuitState::Closed {
                warn!(
                    "Circuit breaker opening after {} consecutive failures",
                    failures
                );
                *state = CircuitState::Open;
            }
        }
    }

    /// Check if requests should be allowed
    fn should_allow_request(&self) -> bool {
        let mut state = self.state.write().unwrap();

        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has elapsed
                if let Some(last_failure) = *self.last_failure_time.read().unwrap() {
                    if last_failure.elapsed() >= self.config.circuit_breaker_timeout {
                        info!(
                            "Circuit breaker transitioning from Open to HalfOpen (timeout elapsed)"
                        );
                        *state = CircuitState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Get current circuit state
    fn get_state(&self) -> CircuitState {
        *self.state.read().unwrap()
    }

    /// Get failure count
    fn get_failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::Relaxed)
    }
}

/// Error recovery manager
#[derive(Clone)]
pub struct RecoveryManager {
    config: RecoveryConfig,
    connection_tracker: Arc<ConnectionTracker>,
    backend_circuit_breakers: Arc<dashmap::DashMap<usize, Arc<CircuitBreaker>>>,
}

impl RecoveryManager {
    /// Create a new recovery manager
    pub fn new(config: RecoveryConfig, connection_tracker: Arc<ConnectionTracker>) -> Self {
        Self {
            config,
            connection_tracker,
            backend_circuit_breakers: Arc::new(dashmap::DashMap::new()),
        }
    }

    /// Get or create circuit breaker for a backend
    fn get_circuit_breaker(&self, backend_index: usize) -> Arc<CircuitBreaker> {
        self.backend_circuit_breakers
            .entry(backend_index)
            .or_insert_with(|| Arc::new(CircuitBreaker::new(self.config.clone())))
            .clone()
    }

    /// Check if a backend is available (circuit not open)
    pub fn is_backend_available(&self, backend_index: usize) -> bool {
        let circuit_breaker = self.get_circuit_breaker(backend_index);
        circuit_breaker.should_allow_request()
    }

    /// Record a successful connection to a backend
    pub fn record_success(&self, backend_index: usize) {
        let circuit_breaker = self.get_circuit_breaker(backend_index);
        circuit_breaker.record_success();
        debug!("Recorded success for backend {}", backend_index);
    }

    /// Record a failure for a backend
    pub fn record_failure(&self, backend_index: usize, error: WebSocketError) {
        let circuit_breaker = self.get_circuit_breaker(backend_index);

        warn!(
            "Recorded {:?} error for backend {} (retryable: {})",
            error,
            backend_index,
            error.is_retryable()
        );

        if error.is_retryable() {
            circuit_breaker.record_failure();
        }
    }

    /// Attempt to reconnect a connection with exponential backoff
    pub async fn attempt_reconnect<F, Fut>(
        &self,
        conn_id: &ConnectionId,
        backend_index: usize,
        connect_fn: F,
    ) -> Result<(), WebSocketError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<(), WebSocketError>>,
    {
        if !self.config.enabled || !self.config.auto_reconnect {
            return Err(WebSocketError::Unknown);
        }

        let mut attempt = 0;
        let mut delay = self.config.initial_retry_delay;

        loop {
            if attempt >= self.config.max_retries {
                error!(
                    "Connection {} failed after {} retry attempts",
                    conn_id, attempt
                );
                return Err(WebSocketError::ConnectionRefused);
            }

            // Check circuit breaker
            if !self.is_backend_available(backend_index) {
                warn!(
                    "Backend {} circuit breaker is open, aborting reconnection attempt",
                    backend_index
                );
                return Err(WebSocketError::ConnectionRefused);
            }

            attempt += 1;

            debug!(
                "Reconnection attempt {} for connection {} (delay: {:?})",
                attempt, conn_id, delay
            );

            // Wait before retrying
            sleep(delay).await;

            // Attempt connection
            match connect_fn().await {
                Ok(()) => {
                    info!(
                        "Connection {} successfully reconnected on attempt {}",
                        conn_id, attempt
                    );
                    self.record_success(backend_index);
                    return Ok(());
                }
                Err(error) => {
                    warn!(
                        "Reconnection attempt {} for connection {} failed: {:?}",
                        attempt, conn_id, error
                    );

                    self.record_failure(backend_index, error.clone());

                    // Check if we should retry
                    if !error.is_retryable() {
                        error!("Non-retryable error {:?}, aborting reconnection", error);
                        return Err(error);
                    }

                    // Exponential backoff
                    delay = Duration::from_millis(
                        (delay.as_millis() as f64 * self.config.backoff_multiplier) as u64,
                    )
                    .min(self.config.max_retry_delay);
                }
            }
        }
    }

    /// Get recovery statistics for a backend
    pub fn get_backend_stats(&self, backend_index: usize) -> BackendRecoveryStats {
        let circuit_breaker = self.get_circuit_breaker(backend_index);
        let last_failure = *circuit_breaker.last_failure_time.read().unwrap();

        BackendRecoveryStats {
            backend_index,
            circuit_state: circuit_breaker.get_state(),
            failure_count: circuit_breaker.get_failure_count(),
            last_failure_time: last_failure,
        }
    }

    /// Get recovery statistics for all backends
    pub fn get_all_stats(&self) -> Vec<BackendRecoveryStats> {
        self.backend_circuit_breakers
            .iter()
            .map(|entry| self.get_backend_stats(*entry.key()))
            .collect()
    }
}

/// Backend recovery statistics
#[derive(Debug, Clone)]
pub struct BackendRecoveryStats {
    pub backend_index: usize,
    pub circuit_state: CircuitState,
    pub failure_count: u32,
    pub last_failure_time: Option<Instant>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_error_retryable() {
        assert!(WebSocketError::ConnectionRefused.is_retryable());
        assert!(WebSocketError::Timeout.is_retryable());
        assert!(WebSocketError::ConnectionReset.is_retryable());
        assert!(!WebSocketError::ProtocolError.is_retryable());
        assert!(!WebSocketError::ClientError.is_retryable());
    }

    #[test]
    fn test_circuit_breaker_normal_operation() {
        let config = RecoveryConfig::default();
        let breaker = CircuitBreaker::new(config);

        assert_eq!(breaker.get_state(), CircuitState::Closed);
        assert!(breaker.should_allow_request());
    }

    #[test]
    fn test_circuit_breaker_opens_on_failures() {
        let config = RecoveryConfig {
            circuit_breaker_threshold: 3,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new(config);

        // Record failures
        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Closed);

        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Closed);

        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Open);
        assert!(!breaker.should_allow_request());
    }

    #[test]
    fn test_circuit_breaker_resets_on_success() {
        let config = RecoveryConfig {
            circuit_breaker_threshold: 3,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new(config);

        // Record failures
        breaker.record_failure();
        breaker.record_failure();

        // Success should reset counter
        breaker.record_success();
        assert_eq!(breaker.get_failure_count(), 0);

        // Need 3 more failures to open
        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Closed);

        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_transition() {
        let config = RecoveryConfig {
            circuit_breaker_threshold: 2,
            circuit_breaker_timeout: Duration::from_millis(50),
            ..Default::default()
        };
        let breaker = CircuitBreaker::new(config);

        // Open the circuit
        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Open);

        // Wait for timeout
        sleep(Duration::from_millis(100)).await;

        // Should transition to HalfOpen
        assert!(breaker.should_allow_request());
        assert_eq!(breaker.get_state(), CircuitState::HalfOpen);

        // Success should close it
        breaker.record_success();
        assert_eq!(breaker.get_state(), CircuitState::Closed);
    }

    #[test]
    fn test_recovery_manager_backend_availability() {
        let tracker = Arc::new(ConnectionTracker::default());
        let config = RecoveryConfig::default();
        let manager = RecoveryManager::new(config, tracker);

        // Initially available
        assert!(manager.is_backend_available(0));

        // Record success
        manager.record_success(0);
        assert!(manager.is_backend_available(0));
    }

    #[test]
    fn test_recovery_manager_circuit_breaker() {
        let tracker = Arc::new(ConnectionTracker::default());
        let config = RecoveryConfig {
            circuit_breaker_threshold: 2,
            ..Default::default()
        };
        let manager = RecoveryManager::new(config, tracker);

        // Record failures to open circuit
        manager.record_failure(0, WebSocketError::ConnectionRefused);
        manager.record_failure(0, WebSocketError::ConnectionRefused);

        // Backend should be unavailable
        assert!(!manager.is_backend_available(0));
    }

    #[tokio::test]
    async fn test_attempt_reconnect_success() {
        let tracker = Arc::new(ConnectionTracker::default());
        let config = RecoveryConfig {
            max_retries: 3,
            initial_retry_delay: Duration::from_millis(10),
            ..Default::default()
        };
        let manager = RecoveryManager::new(config, tracker.clone());

        let conn_id = tracker.register(0, None, None);

        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attempts_clone = attempts.clone();
        let result = manager
            .attempt_reconnect(&conn_id, 0, move || {
                let attempts = attempts_clone.clone();
                async move {
                    let count = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    if count >= 2 {
                        Ok(())
                    } else {
                        Err(WebSocketError::Timeout)
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_attempt_reconnect_max_retries() {
        let tracker = Arc::new(ConnectionTracker::default());
        let config = RecoveryConfig {
            max_retries: 2,
            initial_retry_delay: Duration::from_millis(10),
            ..Default::default()
        };
        let manager = RecoveryManager::new(config, tracker.clone());

        let conn_id = tracker.register(0, None, None);

        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attempts_clone = attempts.clone();
        let result = manager
            .attempt_reconnect(&conn_id, 0, move || {
                let attempts = attempts_clone.clone();
                async move {
                    attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Err(WebSocketError::Timeout)
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
