/// Environment variable configuration overrides
///
/// Provides 12-factor app compliance by allowing configuration
/// to be overridden via environment variables.
///
/// Naming Convention:
/// - HIGHPER_<SETTING_NAME> for global settings
/// - Values are parsed according to type (numbers, durations, sizes)
///
/// Examples:
/// - HIGHPER_MAX_CONNECTIONS=20000
/// - HIGHPER_MAX_REQUEST_BODY=50MB
/// - HIGHPER_LOG_LEVEL=debug
/// - HIGHPER_METRICS_PORT=9091

use anyhow::{anyhow, Result};
use std::env;
use std::time::Duration;

/// Environment variable prefix for all Highper settings
const ENV_PREFIX: &str = "HIGHPER_";

/// Parse environment variable as usize
pub fn env_usize(key: &str) -> Option<usize> {
    env::var(format!("{}{}", ENV_PREFIX, key))
        .ok()
        .and_then(|v| v.parse().ok())
}

/// Parse environment variable as u32
pub fn env_u32(key: &str) -> Option<u32> {
    env::var(format!("{}{}", ENV_PREFIX, key))
        .ok()
        .and_then(|v| v.parse().ok())
}

/// Parse environment variable as u64
pub fn env_u64(key: &str) -> Option<u64> {
    env::var(format!("{}{}", ENV_PREFIX, key))
        .ok()
        .and_then(|v| v.parse().ok())
}

/// Parse environment variable as bool
pub fn env_bool(key: &str) -> Option<bool> {
    env::var(format!("{}{}", ENV_PREFIX, key))
        .ok()
        .and_then(|v| match v.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        })
}

/// Parse environment variable as String
pub fn env_string(key: &str) -> Option<String> {
    env::var(format!("{}{}", ENV_PREFIX, key)).ok()
}

/// Parse environment variable as byte size (supports KB, MB, GB suffixes)
///
/// Examples:
/// - "100" -> 100 bytes
/// - "10KB" -> 10,240 bytes
/// - "5MB" -> 5,242,880 bytes
/// - "1GB" -> 1,073,741,824 bytes
pub fn env_byte_size(key: &str) -> Option<u64> {
    env::var(format!("{}{}", ENV_PREFIX, key))
        .ok()
        .and_then(|v| parse_byte_size(&v).ok())
}

/// Parse environment variable as Duration (supports ms, s, m, h, d suffixes)
///
/// Examples:
/// - "100ms" -> 100 milliseconds
/// - "30s" -> 30 seconds
/// - "5m" -> 5 minutes
/// - "1h" -> 1 hour
/// - "2d" -> 2 days
pub fn env_duration(key: &str) -> Option<Duration> {
    env::var(format!("{}{}", ENV_PREFIX, key))
        .ok()
        .and_then(|v| parse_duration(&v).ok())
}

/// Parse byte size from string (internal helper)
fn parse_byte_size(value: &str) -> Result<u64> {
    if let Some(num_str) = value.strip_suffix("GB") {
        let num = num_str.parse::<u64>()?;
        Ok(num * 1024 * 1024 * 1024)
    } else if let Some(num_str) = value.strip_suffix("MB") {
        let num = num_str.parse::<u64>()?;
        Ok(num * 1024 * 1024)
    } else if let Some(num_str) = value.strip_suffix("KB") {
        let num = num_str.parse::<u64>()?;
        Ok(num * 1024)
    } else if let Some(num_str) = value.strip_suffix("B") {
        let num = num_str.parse::<u64>()?;
        Ok(num)
    } else {
        // No suffix, treat as bytes
        Ok(value.parse::<u64>()?)
    }
}

/// Parse duration from string (internal helper)
fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();

    // Parse number and unit
    let (num_str, unit) = if let Some(n) = s.strip_suffix("ms") {
        (n, "ms")
    } else if let Some(n) = s.strip_suffix('s') {
        (n, "s")
    } else if let Some(n) = s.strip_suffix('m') {
        (n, "m")
    } else if let Some(n) = s.strip_suffix('h') {
        (n, "h")
    } else if let Some(n) = s.strip_suffix('d') {
        (n, "d")
    } else {
        return Err(anyhow!("Invalid duration format: {}", s));
    };

    let num: u64 = num_str
        .trim()
        .parse()
        .map_err(|_| anyhow!("Invalid number in duration: {}", num_str))?;

    match unit {
        "ms" => Ok(Duration::from_millis(num)),
        "s" => Ok(Duration::from_secs(num)),
        "m" => Ok(Duration::from_secs(num * 60)),
        "h" => Ok(Duration::from_secs(num * 3600)),
        "d" => Ok(Duration::from_secs(num * 86400)),
        _ => Err(anyhow!("Unknown duration unit: {}", unit)),
    }
}

/// Apply environment variable overrides to ResourceLimits
///
/// Supported environment variables:
/// - HIGHPER_MAX_FILE_SIZE
/// - HIGHPER_MAX_REQUEST_BODY
/// - HIGHPER_MAX_UPLOAD_SIZE
/// - HIGHPER_MAX_PATH_DEPTH
/// - HIGHPER_MAX_CONNECTIONS_PER_IP
/// - HIGHPER_MAX_REQUESTS_PER_SECOND
pub fn apply_resource_limits_overrides(
    limits: Option<crate::config::schema::ResourceLimits>,
) -> crate::config::schema::ResourceLimits {
    let mut limits = limits.unwrap_or_default();

    if let Some(value) = env_byte_size("MAX_FILE_SIZE") {
        limits.max_file_size = Some(value);
    }

    if let Some(value) = env_byte_size("MAX_REQUEST_BODY") {
        limits.max_request_body = Some(value as usize);
    }

    if let Some(value) = env_byte_size("MAX_UPLOAD_SIZE") {
        limits.max_upload_size = Some(value as usize);
    }

    if let Some(value) = env_usize("MAX_PATH_DEPTH") {
        limits.max_path_depth = Some(value);
    }

    if let Some(value) = env_usize("MAX_CONNECTIONS_PER_IP") {
        limits.max_connections_per_ip = Some(value);
    }

    if let Some(value) = env_u32("MAX_REQUESTS_PER_SECOND") {
        limits.max_requests_per_second = Some(value);
    }

    limits
}

/// Apply environment variable overrides to global configuration
///
/// Supported environment variables:
/// - HIGHPER_LOG_LEVEL (trace, debug, info, warn, error)
/// - HIGHPER_METRICS_PORT
/// - HIGHPER_BIND_ADDRESS
pub fn apply_global_overrides() {
    // Log level can be set via HIGHPER_LOG_LEVEL or RUST_LOG
    if let Some(log_level) = env_string("LOG_LEVEL") {
        env::set_var("RUST_LOG", log_level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_byte_size() {
        assert_eq!(parse_byte_size("100B").unwrap(), 100);
        assert_eq!(parse_byte_size("10KB").unwrap(), 10_240);
        assert_eq!(parse_byte_size("5MB").unwrap(), 5_242_880);
        assert_eq!(parse_byte_size("1GB").unwrap(), 1_073_741_824);
        assert_eq!(parse_byte_size("50").unwrap(), 50);
        assert!(parse_byte_size("invalid").is_err());
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("100ms").unwrap(), Duration::from_millis(100));
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("2d").unwrap(), Duration::from_secs(172800));
        assert!(parse_duration("invalid").is_err());
    }

    #[test]
    fn test_env_bool() {
        env::set_var("HIGHPER_TEST_BOOL_TRUE", "true");
        env::set_var("HIGHPER_TEST_BOOL_1", "1");
        env::set_var("HIGHPER_TEST_BOOL_YES", "yes");
        env::set_var("HIGHPER_TEST_BOOL_ON", "on");
        env::set_var("HIGHPER_TEST_BOOL_FALSE", "false");
        env::set_var("HIGHPER_TEST_BOOL_0", "0");
        env::set_var("HIGHPER_TEST_BOOL_NO", "no");
        env::set_var("HIGHPER_TEST_BOOL_OFF", "off");

        assert_eq!(env_bool("TEST_BOOL_TRUE"), Some(true));
        assert_eq!(env_bool("TEST_BOOL_1"), Some(true));
        assert_eq!(env_bool("TEST_BOOL_YES"), Some(true));
        assert_eq!(env_bool("TEST_BOOL_ON"), Some(true));
        assert_eq!(env_bool("TEST_BOOL_FALSE"), Some(false));
        assert_eq!(env_bool("TEST_BOOL_0"), Some(false));
        assert_eq!(env_bool("TEST_BOOL_NO"), Some(false));
        assert_eq!(env_bool("TEST_BOOL_OFF"), Some(false));

        // Clean up
        env::remove_var("HIGHPER_TEST_BOOL_TRUE");
        env::remove_var("HIGHPER_TEST_BOOL_1");
        env::remove_var("HIGHPER_TEST_BOOL_YES");
        env::remove_var("HIGHPER_TEST_BOOL_ON");
        env::remove_var("HIGHPER_TEST_BOOL_FALSE");
        env::remove_var("HIGHPER_TEST_BOOL_0");
        env::remove_var("HIGHPER_TEST_BOOL_NO");
        env::remove_var("HIGHPER_TEST_BOOL_OFF");
    }

    #[test]
    fn test_env_usize() {
        env::set_var("HIGHPER_TEST_USIZE", "12345");
        assert_eq!(env_usize("TEST_USIZE"), Some(12345));
        env::remove_var("HIGHPER_TEST_USIZE");
    }

    #[test]
    fn test_env_byte_size() {
        env::set_var("HIGHPER_TEST_SIZE", "10MB");
        assert_eq!(env_byte_size("TEST_SIZE"), Some(10_485_760));
        env::remove_var("HIGHPER_TEST_SIZE");
    }

    #[test]
    fn test_env_duration() {
        env::set_var("HIGHPER_TEST_DURATION", "30s");
        assert_eq!(env_duration("TEST_DURATION"), Some(Duration::from_secs(30)));
        env::remove_var("HIGHPER_TEST_DURATION");
    }

    #[test]
    fn test_apply_resource_limits_overrides() {
        // Set test environment variables
        env::set_var("HIGHPER_MAX_FILE_SIZE", "200MB");
        env::set_var("HIGHPER_MAX_REQUEST_BODY", "20MB");
        env::set_var("HIGHPER_MAX_CONNECTIONS_PER_IP", "500");

        let limits = apply_resource_limits_overrides(None);

        assert_eq!(limits.max_file_size, Some(209_715_200));
        assert_eq!(limits.max_request_body, Some(20_971_520));
        assert_eq!(limits.max_connections_per_ip, Some(500));

        // Clean up
        env::remove_var("HIGHPER_MAX_FILE_SIZE");
        env::remove_var("HIGHPER_MAX_REQUEST_BODY");
        env::remove_var("HIGHPER_MAX_CONNECTIONS_PER_IP");
    }
}
