# PHP Compatibility Guide

Comprehensive guide for configuring Highper Gateway limits in coordination with PHP settings for optimal compatibility and error handling.

---

## Table of Contents

1. [Overview](#overview)
2. [Request Body Limits](#request-body-limits)
3. [File Upload Limits](#file-upload-limits)
4. [Execution Timeouts](#execution-timeouts)
5. [Configuration Examples](#configuration-examples)
6. [Best Practices](#best-practices)
7. [Troubleshooting](#troubleshooting)

---

## Overview

Highper Gateway and PHP-FPM each have their own resource limits. **Proper coordination between these limits is critical** for:

1. **Graceful error handling** - PHP provides better error messages
2. **Security** - Gateway acts as outer defense layer
3. **Performance** - Prevent resource exhaustion at gateway level
4. **User experience** - Clear feedback on limit violations

### Layered Limit Architecture

```
┌─────────────────────────────────────────────┐
│          Client Request                      │
└──────────────────┬──────────────────────────┘
                   │
        ┌──────────▼─────────┐
        │  Gateway Limits    │ ← Outer layer (safety net)
        │  (Highper Gateway) │
        └──────────┬──────────┘
                   │
        ┌──────────▼─────────┐
        │  FastCGI Protocol  │ ← Request forwarding
        └──────────┬──────────┘
                   │
        ┌──────────▼─────────┐
        │  PHP-FPM Limits    │ ← Inner layer (enforcement)
        │  (php.ini)         │
        └──────────┬──────────┘
                   │
                   ▼
           PHP Script Execution
```

---

## Request Body Limits

### PHP Setting: `post_max_size`

Controls maximum size of POST data including:
- Form fields
- JSON payloads
- XML data
- File uploads (entire multipart form)

**php.ini**:
```ini
post_max_size = 8M
```

### Gateway Setting: `max_request_body`

Controls maximum request body size at gateway level.

**DSL Configuration**:
```dsl
limits max_request_body=10MB
```

### Coordination Rule

```
Gateway max_request_body >= PHP post_max_size + buffer
```

**Recommended Buffer**: 20-30% extra

**Example**:
```
PHP:     post_max_size = 8M
Gateway: max_request_body = 10MB  (8M + 25% buffer)
```

### Why This Matters

| Scenario | Gateway | PHP | Result |
|----------|---------|-----|--------|
| ✅ Correct | 10 MB | 8 MB | PHP rejects with proper error |
| ❌ Wrong | 5 MB | 8 MB | Gateway rejects prematurely |
| ⚠️ Risky | 8 MB | 8 MB | Race condition, unclear errors |

### GET vs POST Handling

**GET Requests**:
- No body limit validation
- Only URL length matters (usually 8KB browser limit)
- No special handling needed

**POST/PUT/PATCH Requests**:
- Body limit validation applies
- Content-Length header checked
- Rejected before body reading if over limit

---

## File Upload Limits

### PHP Settings

#### `upload_max_filesize`

Maximum size of a **single uploaded file**.

**php.ini**:
```ini
upload_max_filesize = 2M
```

#### `post_max_size`

Maximum size of **entire POST request** (all files + form data).

**php.ini Constraint**:
```ini
upload_max_filesize <= post_max_size
```

### Gateway Setting: `max_upload_size`

Specific limit for file uploads in `multipart/form-data` requests.

**DSL Configuration**:
```dsl
limits max_upload_size=3MB max_request_body=10MB
```

### Coordination Rules

```
1. Gateway max_upload_size >= PHP upload_max_filesize + buffer
2. Gateway max_request_body >= PHP post_max_size + buffer
3. max_upload_size <= max_request_body
```

### Example Configuration

**PHP (php.ini)**:
```ini
upload_max_filesize = 2M
post_max_size = 8M
```

**Gateway (DSL)**:
```dsl
limits max_upload_size=3MB max_request_body=10MB
```

**Validation**:
- ✅ 3MB >= 2M (upload limit coordination)
- ✅ 10MB >= 8M (post limit coordination)
- ✅ 3MB <= 10MB (internal consistency)

### Upload Scenarios

| Upload Size | Files | Post Data | Gateway Check | PHP Check | Result |
|-------------|-------|-----------|---------------|-----------|--------|
| 1.5 MB | 1 file | 100 KB | ✅ Pass (< 3MB) | ✅ Pass (< 2M) | Success |
| 2.5 MB | 1 file | 100 KB | ✅ Pass (< 3MB) | ❌ Fail (> 2M) | PHP error |
| 5 MB | 3 files | 500 KB | ✅ Pass (< 10MB) | ✅ Pass (< 8M) | Success |
| 9 MB | 5 files | 1 MB | ✅ Pass (< 10MB) | ❌ Fail (> 8M) | PHP error |
| 15 MB | 10 files | 2 MB | ❌ Fail (> 10MB) | N/A | Gateway 413 |

---

## Execution Timeouts

### PHP Setting: `max_execution_time`

Maximum time PHP script can execute (in seconds).

**php.ini**:
```ini
max_execution_time = 30
```

**Notes**:
- Applies to script execution time only
- Does NOT include:
  - File upload time
  - External API calls (some cases)
- Can be overridden with `set_time_limit()` in script
- Set to `0` for no limit (dangerous!)

### Gateway Setting: `read_timeout_secs`

FastCGI read timeout - how long gateway waits for PHP response.

**DSL Configuration**:
```dsl
/*.php {
    php_fpm enabled socket="/var/run/php/php-fpm.sock" read_timeout=60s
}
```

### Coordination Rule

```
Gateway read_timeout_secs >= PHP max_execution_time + buffer
```

**Recommended Buffer**: 10-30 seconds extra

**Example**:
```
PHP:     max_execution_time = 30
Gateway: read_timeout = 60s  (30s + 30s buffer)
```

### Why This Matters

| Scenario | Gateway | PHP | Result |
|----------|---------|-----|--------|
| ✅ Correct | 60s | 30s | PHP times out first with proper error |
| ❌ Wrong | 20s | 30s | Gateway times out prematurely |
| ⚠️ Risky | 30s | 30s | Race condition, unclear errors |

### Timeout Scenarios

**Long-Running Operations**:
```ini
# PHP: For batch processing
max_execution_time = 300  ; 5 minutes
```

```dsl
# Gateway: Add buffer
/*.php {
    php_fpm read_timeout=360s  # 6 minutes
}
```

**API Endpoints (Fast)**:
```ini
# PHP: Quick responses
max_execution_time = 10
```

```dsl
# Gateway: Shorter timeout acceptable
/*.php {
    php_fpm read_timeout=30s
}
```

### Additional Timeouts

**Connect Timeout**:
```dsl
/*.php {
    php_fpm connect_timeout=5s  # Time to connect to PHP-FPM
}
```

**Write Timeout**:
```dsl
/*.php {
    php_fpm write_timeout=60s  # Time to send request to PHP
}
```

**Keepalive Timeout**:
```dsl
/*.php {
    php_fpm keepalive_timeout=90s  # Connection reuse duration
}
```

---

## Configuration Examples

### Example 1: Small Application (Default)

**PHP (php.ini)**:
```ini
post_max_size = 8M
upload_max_filesize = 2M
max_execution_time = 30
memory_limit = 128M
```

**Gateway (demo.dsl)**:
```dsl
http://localhost:8080 {
    root "/var/www/app"

    # Coordinated limits (slightly higher than PHP)
    limits max_request_body=10MB max_upload_size=3MB

    /*.php {
        php_fpm enabled socket="/var/run/php/php-fpm.sock"
                pool_size=50
                read_timeout=60s
                connect_timeout=5s
                write_timeout=60s
        proxy localhost:9000
    }
}
```

### Example 2: File Upload Application

**PHP (php.ini)**:
```ini
post_max_size = 100M
upload_max_filesize = 50M
max_execution_time = 300  ; 5 minutes for large uploads
memory_limit = 256M
max_input_time = 300      ; Time to receive POST data
```

**Gateway (upload.dsl)**:
```dsl
http://upload.example.com {
    root "/var/www/uploads"

    # Higher limits for file uploads
    limits max_request_body=120MB max_upload_size=60MB

    /*.php {
        php_fpm enabled socket="/var/run/php/php-fpm.sock"
                pool_size=100
                read_timeout=360s      # 6 min (300s + buffer)
                connect_timeout=10s
                write_timeout=360s     # Large file upload time
        proxy localhost:9000
    }
}
```

### Example 3: API Application (Strict Limits)

**PHP (php.ini)**:
```ini
post_max_size = 1M
upload_max_filesize = 512K
max_execution_time = 10
memory_limit = 64M
```

**Gateway (api.dsl)**:
```dsl
http://api.example.com {
    root "/var/www/api"

    # Strict limits for API
    limits max_request_body=2MB max_upload_size=1MB max_requests_per_second=500

    /*.php {
        php_fpm enabled socket="/var/run/php/php-fpm.sock"
                pool_size=200
                read_timeout=30s
                connect_timeout=3s
                write_timeout=30s
        proxy localhost:9000
    }
}
```

### Example 4: WordPress Application

**PHP (php.ini)**:
```ini
post_max_size = 64M
upload_max_filesize = 32M  ; For media uploads
max_execution_time = 60
memory_limit = 256M
max_input_vars = 3000      ; For large forms
```

**Gateway (wordpress.dsl)**:
```dsl
http://wordpress.example.com {
    root "/var/www/wordpress"
    index index.php

    # WordPress-optimized limits
    limits max_request_body=80MB max_upload_size=40MB max_path_depth=64

    # Admin uploads
    /wp-admin/* {
        limits max_request_body=100MB max_upload_size=50MB
    }

    /*.php {
        php_fpm enabled socket="/var/run/php/php-fpm.sock"
                pool_size=100
                read_timeout=90s
                connect_timeout=5s
                write_timeout=90s
        proxy localhost:9000
    }

    # WordPress rewrite rules
    /* {
        try_files $uri $uri/ /index.php
    }
}
```

---

## Best Practices

### 1. Layer Strategy

**Principle**: Gateway limits should be **slightly higher** than PHP limits.

**Why**:
- PHP provides user-friendly error messages
- PHP errors are logged properly
- Gateway acts as safety net for misconfig

**Anti-Pattern** ❌:
```
Gateway: max_request_body=5MB
PHP:     post_max_size=10M
```
Result: Gateway blocks requests PHP would accept.

**Correct Pattern** ✅:
```
Gateway: max_request_body=12MB (20% buffer)
PHP:     post_max_size=10M
```
Result: PHP validates and provides clear errors.

### 2. Timeout Strategy

**Principle**: Gateway timeouts should be **longer** than PHP timeouts.

**Buffer Recommendations**:
- Fast APIs: +10-20 seconds
- Normal apps: +30 seconds
- Long-running: +50-100% of PHP timeout

**Example**:
```
PHP:     max_execution_time=30
Gateway: read_timeout=60s (100% buffer)
```

### 3. Testing Strategy

**Test Each Limit**:

1. **Below both limits** → Success
2. **Between gateway and PHP** → PHP error (preferred)
3. **Above both limits** → Gateway error

**Upload Testing**:
```bash
# Test file upload
curl -F "file=@large_file.jpg" http://localhost:8080/upload.php

# Test POST data
curl -X POST -H "Content-Type: application/json" \
     -d @large_payload.json \
     http://localhost:8080/api/endpoint
```

### 4. Monitoring Strategy

**Track Both Layers**:
- Gateway 413 errors → Adjust gateway limits
- PHP memory/time errors → Adjust PHP limits
- Frequent PHP errors near limit → Reduce limits

### 5. Documentation Strategy

**Document Both Configurations**:
```dsl
# WordPress installation
# PHP settings: post_max_size=64M, upload_max_filesize=32M, max_execution_time=60
# Gateway adds 25% buffer for graceful PHP error handling
limits max_request_body=80MB max_upload_size=40MB
```

---

## Troubleshooting

### Issue 1: "413 Payload Too Large" at Gateway

**Symptom**: Gateway rejects uploads PHP would accept

**Diagnosis**:
```bash
# Check limits
grep -r "max_request_body\|max_upload_size" config.dsl
php -i | grep "post_max_size\|upload_max_filesize"
```

**Solution**: Increase gateway limits to exceed PHP limits
```dsl
limits max_request_body=100MB  # Was 50MB, PHP allows 80M
```

### Issue 2: PHP "POST Content-Length" Error

**Symptom**: PHP error about exceeding `post_max_size`

**Diagnosis**: Gateway limits are correct, PHP limits too low

**Solution**: Increase PHP limits in php.ini
```ini
post_max_size = 100M  # Was 8M
```
Then restart PHP-FPM:
```bash
sudo systemctl restart php8.2-fpm
```

### Issue 3: Gateway Timeout Before PHP

**Symptom**: 504 Gateway Timeout, PHP script still running

**Diagnosis**:
```bash
# Check timeouts
grep "read_timeout" config.dsl
php -i | grep "max_execution_time"
```

**Solution**: Increase gateway read timeout
```dsl
/*.php {
    php_fpm read_timeout=120s  # Was 30s, PHP allows 60s
}
```

### Issue 4: Slow File Uploads Timing Out

**Symptom**: Large uploads timeout during upload (not processing)

**Diagnosis**: Check write_timeout, not read_timeout

**Solution**:
```dsl
/*.php {
    php_fpm write_timeout=600s  # 10 minutes for large uploads
}
```

Also check PHP's `max_input_time`:
```ini
max_input_time = 600  ; Time to receive POST data
```

### Issue 5: Inconsistent Errors

**Symptom**: Sometimes gateway error, sometimes PHP error

**Diagnosis**: Limits are too close (race condition)

**Solution**: Add minimum 20% buffer between limits
```
Before:
  Gateway: 10MB
  PHP:     10M     ← Too close!

After:
  Gateway: 12MB
  PHP:     10M     ← 20% buffer
```

---

## Quick Reference Table

| Use Case | PHP `post_max_size` | PHP `upload_max_filesize` | Gateway `max_request_body` | Gateway `max_upload_size` | PHP `max_execution_time` | Gateway `read_timeout` |
|----------|---------------------|---------------------------|----------------------------|---------------------------|--------------------------|------------------------|
| Small API | 1M | 512K | 2MB | 1MB | 10s | 30s |
| Web App | 8M | 2M | 10MB | 3MB | 30s | 60s |
| File Upload | 100M | 50M | 120MB | 60MB | 300s | 360s |
| WordPress | 64M | 32M | 80MB | 40MB | 60s | 90s |
| E-commerce | 32M | 10M | 40MB | 12MB | 60s | 90s |

---

## Summary

### Golden Rules

1. **Gateway limits >= PHP limits + buffer**
2. **Test at both layers**
3. **Monitor both error sources**
4. **Document coordinated values**
5. **PHP should enforce, gateway should protect**

### Configuration Checklist

- [ ] Gateway `max_request_body` >= PHP `post_max_size` + 20%
- [ ] Gateway `max_upload_size` >= PHP `upload_max_filesize` + 20%
- [ ] Gateway `read_timeout` >= PHP `max_execution_time` + buffer
- [ ] Gateway `write_timeout` considers upload time
- [ ] PHP `upload_max_filesize` <= PHP `post_max_size`
- [ ] All limits documented together
- [ ] Both layers tested with real requests

### Resources

- [PHP Runtime Configuration](https://www.php.net/manual/en/ini.core.php)
- [PHP-FPM Configuration](https://www.php.net/manual/en/install.fpm.configuration.php)
- [Highper Gateway Configuration](./PRODUCTION_HARDENING.md)

---

**PHP Compatibility Validated** ✅
