//! Request Size Limit Middleware Integration Tests
//!
//! These tests verify that request size limits are properly enforced
//! to prevent memory exhaustion and DoS attacks.
//!
//! Note: These tests focus on the configuration and logic of the limiter.
//! Full end-to-end tests with actual HTTP requests should be done in e2e tests.

use highper_gateway::middleware::request_size_limit::{
    format_byte_size, RequestSizeLimitConfig, RequestSizeLimiter,
};

/// Test default configuration (10MB limit)
#[test]
fn test_default_config() {
    let config = RequestSizeLimitConfig::default();

    assert_eq!(config.max_body_size, 10 * 1024 * 1024); // 10 MB
    assert!(config.enabled);
    assert!(config.error_message.is_none());
}

/// Test byte size formatting
#[test]
fn test_byte_size_formatting() {
    assert_eq!(format_byte_size(0), "0.00 B");
    assert_eq!(format_byte_size(512), "512.00 B");
    assert_eq!(format_byte_size(1024), "1.00 KB");
    assert_eq!(format_byte_size(1536), "1.50 KB");
    assert_eq!(format_byte_size(1024 * 1024), "1.00 MB");
    assert_eq!(format_byte_size(1024 * 1024 * 1024), "1.00 GB");
    assert_eq!(format_byte_size(1024u64 * 1024 * 1024 * 1024), "1.00 TB");

    // Test realistic sizes
    assert_eq!(format_byte_size(10 * 1024 * 1024), "10.00 MB"); // 10 MB
    assert_eq!(format_byte_size(157286400), "150.00 MB"); // ~150 MB
}

/// Test limiter accessor methods
#[test]
fn test_limiter_accessors() {
    let config = RequestSizeLimitConfig {
        max_body_size: 5 * 1024 * 1024, // 5 MB
        enabled: true,
        error_message: Some("Custom error".to_string()),
    };

    let limiter = RequestSizeLimiter::new(config);

    assert_eq!(limiter.max_size(), 5 * 1024 * 1024);
    assert!(limiter.is_enabled());
}

/// Test various size limit configurations
#[test]
fn test_various_size_limits() {
    // Test 1KB limit
    let limiter = RequestSizeLimiter::new(RequestSizeLimitConfig {
        max_body_size: 1024,
        enabled: true,
        error_message: None,
    });
    assert_eq!(limiter.max_size(), 1024);
    assert!(limiter.is_enabled());

    // Test 1MB limit
    let limiter = RequestSizeLimiter::new(RequestSizeLimitConfig {
        max_body_size: 1024 * 1024,
        enabled: true,
        error_message: None,
    });
    assert_eq!(limiter.max_size(), 1024 * 1024);

    // Test 100MB limit
    let limiter = RequestSizeLimiter::new(RequestSizeLimitConfig {
        max_body_size: 100 * 1024 * 1024,
        enabled: true,
        error_message: None,
    });
    assert_eq!(limiter.max_size(), 100 * 1024 * 1024);
}

/// Test when limiter is disabled
#[test]
fn test_limiter_disabled() {
    let config = RequestSizeLimitConfig {
        max_body_size: 1024, // 1 KB limit
        enabled: false,      // Disabled
        error_message: None,
    };

    let limiter = RequestSizeLimiter::new(config);

    assert_eq!(limiter.max_size(), 1024);
    assert!(!limiter.is_enabled());
}

/// Test custom error messages
#[test]
fn test_custom_error_messages() {
    let configs = vec![
        ("Upload size too large. Please reduce file size.", 1024),
        ("File exceeds maximum allowed size.", 5 * 1024 * 1024),
        (
            "Request body is too large for processing.",
            10 * 1024 * 1024,
        ),
    ];

    for (msg, size) in configs {
        let config = RequestSizeLimitConfig {
            max_body_size: size,
            enabled: true,
            error_message: Some(msg.to_string()),
        };

        let limiter = RequestSizeLimiter::new(config.clone());
        assert_eq!(limiter.max_size(), size);
        assert!(limiter.is_enabled());
    }
}

/// Test zero-byte limit (edge case)
#[test]
fn test_zero_byte_limit() {
    let config = RequestSizeLimitConfig {
        max_body_size: 0, // No body allowed
        enabled: true,
        error_message: None,
    };

    let limiter = RequestSizeLimiter::new(config);
    assert_eq!(limiter.max_size(), 0);
    assert!(limiter.is_enabled());
}

/// Test very large limit (max u64)
#[test]
fn test_very_large_limit() {
    let config = RequestSizeLimitConfig {
        max_body_size: u64::MAX, // Practically unlimited
        enabled: true,
        error_message: None,
    };

    let limiter = RequestSizeLimiter::new(config);
    assert_eq!(limiter.max_size(), u64::MAX);
    assert!(limiter.is_enabled());
}

/// Test configuration cloning
#[test]
fn test_config_cloning() {
    let config = RequestSizeLimitConfig {
        max_body_size: 5 * 1024 * 1024,
        enabled: true,
        error_message: Some("Test error".to_string()),
    };

    let cloned = config.clone();

    assert_eq!(cloned.max_body_size, config.max_body_size);
    assert_eq!(cloned.enabled, config.enabled);
    assert_eq!(cloned.error_message, config.error_message);
}

/// Test realistic file upload scenarios configuration
#[test]
fn test_realistic_file_upload_configs() {
    // Image upload API (10MB limit)
    let image_config = RequestSizeLimitConfig {
        max_body_size: 10 * 1024 * 1024,
        enabled: true,
        error_message: Some("Image too large. Maximum size is 10 MB.".to_string()),
    };
    let image_limiter = RequestSizeLimiter::new(image_config);
    assert_eq!(image_limiter.max_size(), 10 * 1024 * 1024);
    assert_eq!(format_byte_size(image_limiter.max_size()), "10.00 MB");

    // Video upload API (100MB limit)
    let video_config = RequestSizeLimitConfig {
        max_body_size: 100 * 1024 * 1024,
        enabled: true,
        error_message: Some("Video too large. Maximum size is 100 MB.".to_string()),
    };
    let video_limiter = RequestSizeLimiter::new(video_config);
    assert_eq!(video_limiter.max_size(), 100 * 1024 * 1024);
    assert_eq!(format_byte_size(video_limiter.max_size()), "100.00 MB");

    // Document upload API (5MB limit)
    let doc_config = RequestSizeLimitConfig {
        max_body_size: 5 * 1024 * 1024,
        enabled: true,
        error_message: Some("Document too large. Maximum size is 5 MB.".to_string()),
    };
    let doc_limiter = RequestSizeLimiter::new(doc_config);
    assert_eq!(doc_limiter.max_size(), 5 * 1024 * 1024);
    assert_eq!(format_byte_size(doc_limiter.max_size()), "5.00 MB");
}

/// Test DoS prevention scenarios - configuration
#[test]
fn test_dos_prevention_configs() {
    // Strict limit for public endpoints
    let public_config = RequestSizeLimitConfig {
        max_body_size: 1024 * 1024, // 1 MB
        enabled: true,
        error_message: Some("Upload too large - DoS protection".to_string()),
    };
    let public_limiter = RequestSizeLimiter::new(public_config);
    assert!(public_limiter.is_enabled());
    assert_eq!(public_limiter.max_size(), 1024 * 1024);

    // More permissive for authenticated endpoints
    let auth_config = RequestSizeLimitConfig {
        max_body_size: 50 * 1024 * 1024, // 50 MB
        enabled: true,
        error_message: None,
    };
    let auth_limiter = RequestSizeLimiter::new(auth_config);
    assert!(auth_limiter.is_enabled());
    assert_eq!(auth_limiter.max_size(), 50 * 1024 * 1024);
}

/// Test configuration validation scenarios
#[test]
fn test_config_validation_scenarios() {
    // Valid configurations
    let configs = vec![
        RequestSizeLimitConfig {
            max_body_size: 1024,
            enabled: true,
            error_message: None,
        },
        RequestSizeLimitConfig {
            max_body_size: 0,
            enabled: true,
            error_message: Some("No body allowed".to_string()),
        },
        RequestSizeLimitConfig {
            max_body_size: u64::MAX,
            enabled: false,
            error_message: None,
        },
    ];

    for config in configs {
        let limiter = RequestSizeLimiter::new(config.clone());
        assert_eq!(limiter.max_size(), config.max_body_size);
        assert_eq!(limiter.is_enabled(), config.enabled);
    }
}

/// Test multiple limiter instances (stateless verification)
#[test]
fn test_multiple_limiters() {
    let limiters: Vec<RequestSizeLimiter> = (0..100)
        .map(|i| {
            let config = RequestSizeLimitConfig {
                max_body_size: (i + 1) * 1024, // 1KB, 2KB, 3KB, ..., 100KB
                enabled: i % 2 == 0,           // Every other one disabled
                error_message: Some(format!("Error for limiter {}", i)),
            };
            RequestSizeLimiter::new(config)
        })
        .collect();

    // Verify all limiters created correctly
    assert_eq!(limiters.len(), 100);

    for (i, limiter) in limiters.iter().enumerate() {
        assert_eq!(limiter.max_size(), ((i + 1) * 1024) as u64);
        assert_eq!(limiter.is_enabled(), i % 2 == 0);
    }
}

/// Test byte formatting edge cases
#[test]
fn test_byte_formatting_edge_cases() {
    // Exact boundaries
    assert_eq!(format_byte_size(1024), "1.00 KB");
    assert_eq!(format_byte_size(1024 * 1024), "1.00 MB");
    assert_eq!(format_byte_size(1024 * 1024 * 1024), "1.00 GB");

    // Just below boundaries
    assert_eq!(format_byte_size(1023), "1023.00 B");
    assert_eq!(format_byte_size(1024 * 1024 - 1), "1024.00 KB");

    // Just above boundaries
    assert_eq!(format_byte_size(1025), "1.00 KB");
    assert_eq!(format_byte_size(1024 * 1024 + 1), "1.00 MB");

    // Common file sizes
    assert_eq!(format_byte_size(500 * 1024), "500.00 KB"); // 500 KB file
    assert_eq!(format_byte_size(2 * 1024 * 1024), "2.00 MB"); // 2 MB image
    assert_eq!(format_byte_size(50 * 1024 * 1024), "50.00 MB"); // 50 MB video
}

/// Test configuration patterns for different API types
#[test]
fn test_api_type_configurations() {
    // REST API - Small JSON payloads
    let rest_config = RequestSizeLimitConfig {
        max_body_size: 1024 * 1024, // 1 MB
        enabled: true,
        error_message: Some("Request payload too large for API".to_string()),
    };
    assert_eq!(RequestSizeLimiter::new(rest_config).max_size(), 1024 * 1024);

    // GraphQL API - Potentially larger queries
    let graphql_config = RequestSizeLimitConfig {
        max_body_size: 5 * 1024 * 1024, // 5 MB
        enabled: true,
        error_message: Some("GraphQL query too large".to_string()),
    };
    assert_eq!(
        RequestSizeLimiter::new(graphql_config).max_size(),
        5 * 1024 * 1024
    );

    // File Upload API - Large files allowed
    let upload_config = RequestSizeLimitConfig {
        max_body_size: 100 * 1024 * 1024, // 100 MB
        enabled: true,
        error_message: Some("File upload too large".to_string()),
    };
    assert_eq!(
        RequestSizeLimiter::new(upload_config).max_size(),
        100 * 1024 * 1024
    );

    // Webhook endpoint - Moderate size
    let webhook_config = RequestSizeLimitConfig {
        max_body_size: 10 * 1024 * 1024, // 10 MB
        enabled: true,
        error_message: Some("Webhook payload too large".to_string()),
    };
    assert_eq!(
        RequestSizeLimiter::new(webhook_config).max_size(),
        10 * 1024 * 1024
    );
}

/// Test limiter creation performance
#[test]
fn test_limiter_creation_performance() {
    use std::time::Instant;

    let iterations = 10000;
    let start = Instant::now();

    for i in 0..iterations {
        let config = RequestSizeLimitConfig {
            max_body_size: (i + 1) * 1024,
            enabled: true,
            error_message: None,
        };
        let _ = RequestSizeLimiter::new(config);
    }

    let duration = start.elapsed();
    let per_creation = duration.as_nanos() / iterations as u128;

    println!("Average time per limiter creation: {} ns", per_creation);

    // Should be very fast (< 10 microseconds per creation)
    assert!(
        per_creation < 10_000,
        "Limiter creation should be extremely fast"
    );
}

/// Test configuration with extreme values
#[test]
fn test_extreme_values() {
    // Minimum value
    let min_config = RequestSizeLimiter::new(RequestSizeLimitConfig {
        max_body_size: 0,
        enabled: true,
        error_message: None,
    });
    assert_eq!(min_config.max_size(), 0);

    // Maximum value
    let max_config = RequestSizeLimiter::new(RequestSizeLimitConfig {
        max_body_size: u64::MAX,
        enabled: true,
        error_message: None,
    });
    assert_eq!(max_config.max_size(), u64::MAX);

    // Common extreme cases
    let petabyte = 1024u64 * 1024 * 1024 * 1024 * 1024; // 1 PB
    let pb_config = RequestSizeLimiter::new(RequestSizeLimitConfig {
        max_body_size: petabyte,
        enabled: true,
        error_message: None,
    });
    assert_eq!(pb_config.max_size(), petabyte);
}
