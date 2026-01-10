# Production Hardening Guide

Comprehensive guide for deploying Highper Gateway webserver features in production with enterprise-grade security, reliability, and observability.

---

## Table of Contents

1. [Overview](#overview)
2. [Security Hardening](#security-hardening)
3. [Resource Limits](#resource-limits)
4. [Observability & Monitoring](#observability--monitoring)
5. [Configuration Best Practices](#configuration-best-practices)
6. [Deployment Checklist](#deployment-checklist)
7. [Troubleshooting](#troubleshooting)

---

## Overview

This guide covers the production hardening features implemented for the Highper Gateway webserver:

- **Security Validation**: Path traversal prevention, file access controls, injection protection
- **Resource Limits**: Connection limits, rate limiting, memory/FD quotas
- **Observability**: Comprehensive metrics, logging, health checks
- **Error Handling**: Graceful degradation, custom error pages, detailed logging

### Architecture

```
┌─────────────────────────────────────────────────────┐
│              Incoming HTTP Request                   │
└──────────────────┬──────────────────────────────────┘
                   │
        ┌──────────▼─────────┐
        │  Connection Limiter │ ← Global/Per-IP limits
        └──────────┬──────────┘
                   │
        ┌──────────▼─────────┐
        │   Rate Limiter      │ ← Token bucket algorithm
        └──────────┬──────────┘
                   │
        ┌──────────▼─────────┐
        │  Path Validator     │ ← Traversal, null bytes
        └──────────┬──────────┘
                   │
        ┌──────────▼─────────┐
        │  File ACL Checker   │ ← Sensitive/hidden files
        └──────────┬──────────┘
                   │
        ┌──────────▼─────────┐
        │  Size Validator     │ ← File/body size limits
        └──────────┬──────────┘
                   │
        ┌──────────▼─────────┐
        │  PHP Validator      │ ← Script validation (if PHP)
        └──────────┬──────────┘
                   │
        ┌──────────▼─────────┐
        │ FastCGI Sanitizer   │ ← Parameter injection protection
        └──────────┬──────────┘
                   │
        ┌──────────▼─────────┐
        │  Metrics Recorder   │ ← All events logged
        └──────────┬──────────┘
                   │
                   ▼
           Response to Client
```

---

## Security Hardening

### Path Validation

**Module**: `src/webserver/security.rs`

#### Features

1. **Path Traversal Prevention**
   - URL decoding validation
   - Null byte detection (`\0` in paths)
   - Directory traversal pattern blocking (`..` sequences)
   - Path canonicalization enforcement
   - Document root boundary checks

2. **Sensitive File Blocking**
   - Environment files: `.env`, `.env.local`, `.env.production`
   - Version control: `.git`, `.gitignore`, `.gitconfig`
   - Server configs: `.htaccess`, `.htpasswd`, `web.config`
   - SSH keys: `id_rsa`, `id_dsa`, `id_ecdsa`, `authorized_keys`
   - Package configs: `.npmrc`, `composer.json`, `composer.lock`

3. **Dangerous Extension Blocking**
   - Shell scripts: `.sh`, `.bash`, `.zsh`, `.fish`
   - Executables: `.exe`, `.dll`, `.so`, `.dylib`
   - Windows scripts: `.bat`, `.cmd`, `.ps1`
   - Script languages: `.py`, `.rb`, `.pl`, `.lua` (unless explicitly allowed)

4. **Hidden File Protection**
   - Files starting with `.` (except `.` and `..`)
   - Prevents information disclosure

5. **Path Depth Limiting**
   - Maximum path depth: **32 levels**
   - Prevents deep directory DoS attacks

#### Configuration

```rust
// Built-in limits (constants in security.rs)
pub const MAX_STATIC_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100 MB
pub const MAX_PHP_REQUEST_BODY: usize = 10 * 1024 * 1024; // 10 MB
pub const MAX_PATH_DEPTH: usize = 32;
```

#### Usage Example

```rust
use highper_gateway::webserver::security::PathValidator;

let validator = PathValidator::new("/var/www/html");

// Validate request path
match validator.validate_path("/api/users") {
    Ok(safe_path) => {
        // Path is safe, proceed
        println!("Validated path: {:?}", safe_path);
    }
    Err(e) => {
        // Security violation detected
        warn!("Path validation failed: {}", e);
        return forbidden_response();
    }
}

// Check file permissions
match validator.is_file_allowed(&path) {
    Ok(()) => { /* File is allowed */ }
    Err(e) => {
        warn!("File access denied: {}", e);
        return forbidden_response();
    }
}
```

#### Security Responses

| Violation | HTTP Status | Action |
|-----------|-------------|--------|
| Path traversal | 403 Forbidden | Block request, log warning |
| Sensitive file | 403 Forbidden | Block request, log warning |
| Dangerous extension | 403 Forbidden | Block request, log warning |
| Hidden file | 403 Forbidden | Block request, log warning |
| Oversized file | 413 Payload Too Large | Block request, log warning |
| Invalid PHP script | 403 Forbidden | Block request, log warning |

### FastCGI Parameter Sanitization

**Function**: `sanitize_fastcgi_param()`

#### Protection

- Removes control characters (except `\n`, `\r`, `\t`)
- Prevents FastCGI injection attacks
- Applied to:
  - REQUEST_URI
  - QUERY_STRING
  - HTTP headers
  - SERVER_NAME
  - Content-Type/Content-Length

#### Example

```rust
use highper_gateway::webserver::security::sanitize_fastcgi_param;

let safe_uri = sanitize_fastcgi_param(request_uri);
let safe_query = sanitize_fastcgi_param(query_string);
```

---

## Resource Limits

**Module**: `src/webserver/resource_limits.rs`

### Connection Limiting

#### Features

- **Global connection limit**: Prevents server overload
- **Per-IP connection limit**: Prevents client monopolization
- **RAII guards**: Automatic cleanup on connection close
- **Atomic counters**: Lock-free performance

#### Configuration

```rust
let config = ResourceLimitsConfig {
    max_concurrent_connections: 10_000,    // Global limit
    max_connections_per_ip: 100,           // Per-IP limit
    ..Default::default()
};

let limiter = ConnectionLimiter::new(config);
```

#### Usage

```rust
// Check if connection is allowed
match limiter.check_connection(client_ip) {
    Ok(guard) => {
        // Connection allowed, guard will cleanup on drop
        handle_connection().await;
        // Guard automatically decrements counters on drop
    }
    Err(e) => {
        // Connection rejected
        return connection_limit_exceeded();
    }
}
```

#### Monitoring

```rust
let global_count = limiter.global_connection_count();
let ip_count = limiter.ip_connection_count("192.168.1.1");

println!("Global connections: {}/{}", global_count, config.max_concurrent_connections);
println!("IP connections: {}/{}", ip_count, config.max_connections_per_ip);
```

### Rate Limiting

#### Features

- **Token bucket algorithm**: Industry-standard implementation
- **Per-IP rate limits**: Prevents request flooding
- **Global rate limits**: Protects overall capacity
- **Automatic token refill**: Gradual capacity restoration
- **Bucket cleanup**: Periodic removal of expired entries

#### Configuration

```rust
let config = ResourceLimitsConfig {
    max_requests_per_second_per_ip: 100,    // Per-IP limit
    max_requests_per_second_global: 10_000, // Global limit
    rate_limit_window: Duration::from_secs(1),
    ..Default::default()
};

let rate_limiter = RateLimiter::new(config);
```

#### Usage

```rust
// Check rate limit before processing request
match rate_limiter.check_rate_limit(client_ip) {
    Ok(()) => {
        // Request allowed
        handle_request().await;
    }
    Err(e) => {
        // Rate limit exceeded
        return too_many_requests();
    }
}

// Periodic cleanup (run in background task)
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        rate_limiter.cleanup_expired_buckets(Duration::from_secs(300));
    }
});
```

### File Descriptor Limiting

#### Features

- **Max open files**: Prevents FD exhaustion
- **RAII guards**: Automatic cleanup
- **Real-time tracking**: Current usage monitoring

#### Configuration

```rust
let config = ResourceLimitsConfig {
    max_open_files: 1_000,
    ..Default::default()
};

let fd_limiter = FileDescriptorLimiter::new(config);
```

#### Usage

```rust
// Check before opening file
match fd_limiter.check_file_open() {
    Ok(guard) => {
        let file = tokio::fs::File::open(&path).await?;
        // Process file
        // Guard automatically decrements counter on drop
    }
    Err(e) => {
        return service_unavailable();
    }
}
```

### Memory Limiting

#### Features

- **Request buffer limits**: Prevents memory exhaustion
- **Per-request tracking**: Granular accounting
- **Automatic cleanup**: Memory release on drop

#### Configuration

```rust
let config = ResourceLimitsConfig {
    max_request_buffer_memory: 100 * 1024 * 1024, // 100 MB
    ..Default::default()
};

let memory_limiter = MemoryLimiter::new(config);
```

#### Usage

```rust
// Check before allocating memory
let buffer_size = 1024 * 1024; // 1 MB
match memory_limiter.check_memory_allocation(buffer_size) {
    Ok(guard) => {
        let buffer = Vec::with_capacity(buffer_size);
        // Use buffer
        // Guard automatically releases memory on drop
    }
    Err(e) => {
        return insufficient_resources();
    }
}
```

---

## Observability & Monitoring

**Module**: `src/webserver/observability.rs`

### Metrics

#### Request Metrics

```rust
let metrics = Arc::new(WebserverMetrics::new());

// Track requests
metrics.record_request("/api/users");
metrics.record_static_file_request();
metrics.record_php_request();
metrics.record_directory_listing_request();
```

#### Response Metrics

```rust
// Track responses
metrics.record_success();
metrics.record_error(404, "/not-found");
```

#### Security Metrics

```rust
// Track security violations
metrics.record_path_traversal_attempt("../etc/passwd");
metrics.record_sensitive_file_access(".env");
metrics.record_hidden_file_access(".htaccess");
metrics.record_oversized_file(200_000_000, 100_000_000);
metrics.record_invalid_php_script("/scripts/bad.txt");
metrics.record_fastcgi_injection_attempt("param\\x00value");
```

#### Resource Limit Metrics

```rust
// Track resource rejections
metrics.record_connection_limit_rejection("192.168.1.1");
metrics.record_rate_limit_rejection("192.168.1.2");
metrics.record_file_descriptor_limit_rejection();
metrics.record_memory_limit_rejection(10_000_000);
```

#### Performance Metrics

```rust
// Track performance
metrics.record_bytes_served(1024, "/api/users");
metrics.record_request_duration(Duration::from_millis(50), "/api/users");
metrics.record_cache_hit();
metrics.record_cache_miss();
```

### Metrics Snapshot

```rust
let snapshot = metrics.get_snapshot();

println!("Total requests: {}", snapshot.total_requests);
println!("Error rate: {:.2}%", snapshot.error_rate() * 100.0);
println!("Avg duration: {:.2}ms", snapshot.avg_request_duration_ms());
println!("Cache hit rate: {:.2}%", snapshot.cache_hit_rate() * 100.0);
println!("Security violations: {}", snapshot.total_security_violations());
println!("Resource rejections: {}", snapshot.total_resource_limit_rejections());
```

### Per-Path Metrics

```rust
// Get metrics for specific path
if let Some(path_metrics) = metrics.get_path_metrics("/api/users") {
    println!("Path: {}", path_metrics.path);
    println!("Requests: {}", path_metrics.request_count);
    println!("Avg duration: {:.2}ms", path_metrics.avg_duration_ms());
    println!("Error rate: {:.2}%", path_metrics.error_rate() * 100.0);
    println!("Bytes served: {}", path_metrics.bytes_served);
}

// Get top paths
let top_paths = metrics.get_top_paths(10);
for (i, path) in top_paths.iter().enumerate() {
    println!("{}. {} - {} requests", i + 1, path.path, path.request_count);
}
```

### Health Checks

```rust
let mut health = HealthStatus::new();

// Add checks
health.add_check(
    "database".to_string(),
    "ok".to_string(),
    None
);

health.add_check(
    "php_fpm".to_string(),
    "ok".to_string(),
    Some("Pool: 50/100 connections".to_string())
);

println!("Healthy: {}", health.healthy);
println!("Checks: {}", health.checks.len());
```

### Prometheus Integration Example

```rust
// Export metrics to Prometheus
fn export_metrics(metrics: &WebserverMetrics) -> String {
    let snapshot = metrics.get_snapshot();

    format!(
        "# HELP http_requests_total Total number of HTTP requests\n\
         # TYPE http_requests_total counter\n\
         http_requests_total{{}} {}\n\
         # HELP http_errors_total Total number of HTTP errors\n\
         # TYPE http_errors_total counter\n\
         http_errors_total{{}} {}\n\
         # HELP security_violations_total Total security violations\n\
         # TYPE security_violations_total counter\n\
         security_violations_total{{}} {}\n",
        snapshot.total_requests,
        snapshot.error_responses,
        snapshot.total_security_violations()
    )
}
```

---

## Configuration Best Practices

### Development Environment

```rust
let config = ResourceLimitsConfig {
    max_concurrent_connections: 100,
    max_connections_per_ip: 10,
    max_requests_per_second_per_ip: 50,
    max_requests_per_second_global: 1_000,
    max_open_files: 100,
    max_request_buffer_memory: 50 * 1024 * 1024,
    ..Default::default()
};
```

### Production Environment (Small)

```rust
let config = ResourceLimitsConfig {
    max_concurrent_connections: 5_000,
    max_connections_per_ip: 50,
    max_requests_per_second_per_ip: 100,
    max_requests_per_second_global: 5_000,
    max_open_files: 500,
    max_request_buffer_memory: 100 * 1024 * 1024,
    ..Default::default()
};
```

### Production Environment (Large)

```rust
let config = ResourceLimitsConfig {
    max_concurrent_connections: 20_000,
    max_connections_per_ip: 200,
    max_requests_per_second_per_ip: 200,
    max_requests_per_second_global: 20_000,
    max_open_files: 2_000,
    max_request_buffer_memory: 500 * 1024 * 1024,
    ..Default::default()
};
```

### High-Traffic CDN Environment

```rust
let config = ResourceLimitsConfig {
    max_concurrent_connections: 100_000,
    max_connections_per_ip: 500,
    max_requests_per_second_per_ip: 500,
    max_requests_per_second_global: 100_000,
    max_open_files: 10_000,
    max_request_buffer_memory: 1 * 1024 * 1024 * 1024, // 1 GB
    ..Default::default()
};
```

---

## Deployment Checklist

### Pre-Deployment

- [ ] Review and configure resource limits for your traffic profile
- [ ] Set appropriate file size limits (`MAX_STATIC_FILE_SIZE`)
- [ ] Configure PHP request body limits (`MAX_PHP_REQUEST_BODY`)
- [ ] Test security validations with sample attacks
- [ ] Set up monitoring/metrics collection
- [ ] Configure health check endpoints
- [ ] Review sensitive files list and add custom entries if needed
- [ ] Test error pages (404, 403, 500)

### Deployment

- [ ] Enable rate limiting
- [ ] Enable connection limiting
- [ ] Enable file descriptor limiting
- [ ] Enable memory limiting
- [ ] Start metrics collection
- [ ] Configure log aggregation
- [ ] Set up alerts for security violations
- [ ] Set up alerts for resource limit rejections
- [ ] Configure dashboard (Grafana, etc.)

### Post-Deployment

- [ ] Monitor error rates
- [ ] Monitor security violation rates
- [ ] Monitor resource utilization
- [ ] Tune limits based on actual traffic
- [ ] Review logs for anomalies
- [ ] Test failover scenarios
- [ ] Document incident response procedures

---

## Troubleshooting

### High Security Violation Rate

**Symptom**: Many `path_traversal_attempts` or `sensitive_file_access_attempts`

**Causes**:
- Actual attack in progress
- Misconfigured client application
- Broken URL generation

**Actions**:
1. Check logs for IP addresses
2. Analyze attack patterns
3. Consider blocking IPs via firewall
4. Review application URL generation

### High Connection/Rate Limit Rejections

**Symptom**: Many legitimate users getting 429/503 errors

**Causes**:
- Limits too restrictive for traffic pattern
- Traffic spike
- DDoS attack

**Actions**:
1. Review current limits vs. actual traffic
2. Increase limits if legitimate traffic
3. Implement IP whitelisting for known clients
4. Use CDN/load balancer for traffic distribution

### File Descriptor Exhaustion

**Symptom**: `file_descriptor_limit_rejections` increasing

**Causes**:
- `max_open_files` too low
- File handle leaks (shouldn't happen with RAII guards)
- Very high concurrent static file requests

**Actions**:
1. Increase `max_open_files` limit
2. Check system-level file descriptor limits (`ulimit -n`)
3. Review file serving patterns
4. Consider connection pooling or CDN

### Memory Pressure

**Symptom**: `memory_limit_rejections` or OOM errors

**Causes**:
- `max_request_buffer_memory` too low
- Very large request bodies
- Memory leak (shouldn't happen with RAII guards)

**Actions**:
1. Increase `max_request_buffer_memory`
2. Review system memory availability
3. Consider request streaming for large uploads
4. Review PHP-FPM memory limits

### Performance Degradation

**Symptom**: Increasing average request duration

**Causes**:
- Resource contention
- Approaching connection limits
- Disk I/O bottleneck
- PHP-FPM worker exhaustion

**Actions**:
1. Review per-path metrics for slow endpoints
2. Check system resources (CPU, memory, disk I/O)
3. Scale PHP-FPM workers
4. Consider caching strategies
5. Use CDN for static assets

---

## Best Practices

### Security

1. **Defense in Depth**: Multiple validation layers
2. **Fail Closed**: Deny by default, allow explicitly
3. **Least Privilege**: Minimal file access permissions
4. **Logging**: Log all security violations
5. **Regular Updates**: Keep security lists updated

### Performance

1. **Lock-Free**: Use atomic operations
2. **Minimal Allocations**: Reuse buffers
3. **Lazy Loading**: Initialize resources on demand
4. **Early Rejection**: Fail fast for invalid requests
5. **Caching**: Cache validation results where possible

### Observability

1. **Metrics First**: Instrument everything
2. **Structured Logging**: Use consistent log formats
3. **Distributed Tracing**: Track request flows
4. **Alerting**: Set up proactive alerts
5. **Dashboards**: Visualize key metrics

### Operations

1. **Gradual Rollout**: Deploy changes incrementally
2. **Canary Testing**: Test on small traffic percentage
3. **Rollback Plan**: Have quick rollback procedure
4. **Documentation**: Document all configurations
5. **Runbooks**: Create incident response guides

---

## Appendix

### Security Validation Tests

All security features have comprehensive test coverage:

```bash
# Run security tests
cargo test --lib webserver::security

# Run resource limit tests
cargo test --lib webserver::resource_limits

# Run observability tests
cargo test --lib webserver::observability

# Run all webserver integration tests
cargo test --test webserver_integration_tests
```

### Metrics Reference

See `MetricsSnapshot` in `src/webserver/observability.rs` for complete list of available metrics.

### Configuration Reference

See `ResourceLimitsConfig` in `src/webserver/resource_limits.rs` for all configurable limits.

---

**Production Hardening Complete** ✅

All features tested and ready for enterprise deployment.
