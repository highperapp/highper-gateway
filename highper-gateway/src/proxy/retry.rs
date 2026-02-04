//! Retry logic with exponential backoff
//!
//! Provides resilient upstream communication with configurable retry strategies.

use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier (exponential growth factor)
    pub multiplier: f64,
    /// Add random jitter to prevent thundering herd
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

/// Retry strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryStrategy {
    /// Fixed delay between retries
    Fixed,
    /// Exponential backoff with optional jitter
    Exponential,
    /// Linear backoff (delay increases linearly)
    Linear,
}

/// Retry policy determines when to retry
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Retry on connection errors
    pub retry_on_connection_error: bool,
    /// Retry on timeout
    pub retry_on_timeout: bool,
    /// Retry on specific status codes (5xx by default)
    pub retry_on_status_codes: Vec<u16>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            retry_on_connection_error: true,
            retry_on_timeout: true,
            retry_on_status_codes: vec![500, 502, 503, 504],
        }
    }
}

/// Retry executor
pub struct RetryExecutor {
    config: RetryConfig,
    strategy: RetryStrategy,
    policy: RetryPolicy,
}

impl RetryExecutor {
    /// Create a new retry executor
    pub fn new(config: RetryConfig, strategy: RetryStrategy, policy: RetryPolicy) -> Self {
        Self {
            config,
            strategy,
            policy,
        }
    }

    /// Create with default configuration
    pub fn default_retry() -> Self {
        Self::new(
            RetryConfig::default(),
            RetryStrategy::Exponential,
            RetryPolicy::default(),
        )
    }

    /// Execute a function with retry logic
    pub async fn execute<F, Fut, T, E>(&self, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut attempt = 0;

        loop {
            attempt += 1;

            match operation().await {
                Ok(result) => {
                    if attempt > 1 {
                        debug!("Operation succeeded after {} attempts", attempt);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    if attempt >= self.config.max_attempts {
                        warn!(
                            "Operation failed after {} attempts: {}",
                            attempt, e
                        );
                        return Err(e);
                    }

                    let backoff = self.calculate_backoff(attempt);
                    debug!(
                        "Attempt {} failed: {}. Retrying in {:?}",
                        attempt, e, backoff
                    );

                    sleep(backoff).await;
                }
            }
        }
    }

    /// Calculate backoff duration based on strategy
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base_duration = match self.strategy {
            RetryStrategy::Fixed => self.config.initial_backoff,
            RetryStrategy::Exponential => {
                let multiplier = self.config.multiplier.powi(attempt as i32 - 1);
                Duration::from_secs_f64(
                    self.config.initial_backoff.as_secs_f64() * multiplier
                )
            }
            RetryStrategy::Linear => {
                Duration::from_secs_f64(
                    self.config.initial_backoff.as_secs_f64() * attempt as f64
                )
            }
        };

        // Cap at max_backoff
        let duration = base_duration.min(self.config.max_backoff);

        // Add jitter if enabled
        if self.config.jitter {
            self.add_jitter(duration)
        } else {
            duration
        }
    }

    /// Add random jitter to duration (±25%)
    fn add_jitter(&self, duration: Duration) -> Duration {
        use std::time::SystemTime;

        let nanos = duration.as_nanos() as u64;
        let jitter_range = nanos / 4; // ±25%

        // Use system time as pseudo-random seed
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let jitter = (seed % (jitter_range * 2)).saturating_sub(jitter_range);
        let adjusted_nanos = (nanos as i64 + jitter as i64).max(0) as u64;

        Duration::from_nanos(adjusted_nanos)
    }

    /// Check if error should be retried based on policy
    pub fn should_retry(&self, status_code: Option<u16>, is_network_error: bool) -> bool {
        // Check network/connection errors
        if is_network_error && self.policy.retry_on_connection_error {
            return true;
        }

        // Check status codes
        if let Some(code) = status_code {
            return self.policy.retry_on_status_codes.contains(&code);
        }

        false
    }
}

/// Retry result wrapper
#[derive(Debug)]
pub enum RetryResult<T> {
    /// Operation succeeded
    Success(T),
    /// Operation failed after all retries
    Failed(String),
    /// Operation should not be retried
    NoRetry(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: false,
        };

        let executor = RetryExecutor::new(
            config,
            RetryStrategy::Exponential,
            RetryPolicy::default(),
        );

        let result = executor
            .execute(|| async {
                let count = counter_clone.fetch_add(1, Ordering::Relaxed);
                if count < 2 {
                    Err("Temporary failure")
                } else {
                    Ok("Success")
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Success");
        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: false,
        };

        let executor = RetryExecutor::new(
            config,
            RetryStrategy::Exponential,
            RetryPolicy::default(),
        );

        let result = executor
            .execute(|| async {
                counter_clone.fetch_add(1, Ordering::Relaxed);
                Err::<(), _>("Always fails")
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_immediate_success() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let executor = RetryExecutor::default_retry();

        let result = executor
            .execute(|| async {
                counter_clone.fetch_add(1, Ordering::Relaxed);
                Ok::<_, String>("Success")
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_backoff_calculation() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(5),
            multiplier: 2.0,
            jitter: false,
        };

        let executor = RetryExecutor::new(
            config,
            RetryStrategy::Exponential,
            RetryPolicy::default(),
        );

        // Test exponential backoff
        assert_eq!(executor.calculate_backoff(1), Duration::from_millis(100));
        assert_eq!(executor.calculate_backoff(2), Duration::from_millis(200));
        assert_eq!(executor.calculate_backoff(3), Duration::from_millis(400));
        assert_eq!(executor.calculate_backoff(4), Duration::from_millis(800));
    }

    #[test]
    fn test_linear_backoff() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(5),
            multiplier: 2.0,
            jitter: false,
        };

        let executor = RetryExecutor::new(
            config,
            RetryStrategy::Linear,
            RetryPolicy::default(),
        );

        assert_eq!(executor.calculate_backoff(1), Duration::from_millis(100));
        assert_eq!(executor.calculate_backoff(2), Duration::from_millis(200));
        assert_eq!(executor.calculate_backoff(3), Duration::from_millis(300));
    }

    #[test]
    fn test_fixed_backoff() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(5),
            multiplier: 2.0,
            jitter: false,
        };

        let executor = RetryExecutor::new(
            config,
            RetryStrategy::Fixed,
            RetryPolicy::default(),
        );

        assert_eq!(executor.calculate_backoff(1), Duration::from_millis(100));
        assert_eq!(executor.calculate_backoff(2), Duration::from_millis(100));
        assert_eq!(executor.calculate_backoff(3), Duration::from_millis(100));
    }

    #[test]
    fn test_should_retry() {
        let executor = RetryExecutor::default_retry();

        // Should retry on 5xx status codes
        assert!(executor.should_retry(Some(500), false));
        assert!(executor.should_retry(Some(502), false));
        assert!(executor.should_retry(Some(503), false));
        assert!(executor.should_retry(Some(504), false));

        // Should not retry on 4xx status codes
        assert!(!executor.should_retry(Some(400), false));
        assert!(!executor.should_retry(Some(404), false));

        // Should retry on network errors
        assert!(executor.should_retry(None, true));

        // Should not retry if no error condition
        assert!(!executor.should_retry(Some(200), false));
    }

    #[test]
    fn test_max_backoff_cap() {
        let config = RetryConfig {
            max_attempts: 10,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: false,
        };

        let executor = RetryExecutor::new(
            config,
            RetryStrategy::Exponential,
            RetryPolicy::default(),
        );

        // After many attempts, backoff should be capped at max_backoff
        let backoff = executor.calculate_backoff(10);
        assert!(backoff <= Duration::from_secs(1));
    }
}
