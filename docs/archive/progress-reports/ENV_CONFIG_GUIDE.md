# Environment Variable Configuration Guide

Guide for configuring Highper Gateway using environment variables for 12-factor app compliance.

---

## Overview

Highper Gateway supports configuration through environment variables, enabling:

1. **12-Factor App Compliance** - Store config in the environment
2. **Container-Friendly** - Easy configuration in Docker/Kubernetes
3. **CI/CD Integration** - Override settings per environment without code changes
4. **Zero-Downtime Updates** - Change limits without redeploying binaries

### Configuration Priority

Settings are applied in this order (highest priority first):

```
1. Environment Variables (HIGHPER_*)
2. DSL Configuration File
3. Built-in Defaults
```

---

## Resource Limits

### HIGHPER_MAX_FILE_SIZE

**Description**: Maximum static file size

**Type**: Byte size (supports B, KB, MB, GB suffixes)

**Default**: 100MB

**Examples**:
```bash
export HIGHPER_MAX_FILE_SIZE=200MB
export HIGHPER_MAX_FILE_SIZE=1GB
export HIGHPER_MAX_FILE_SIZE=524288000  # 500MB in bytes
```

---

### HIGHPER_MAX_REQUEST_BODY

**Description**: Maximum HTTP request body size (POST/PUT/PATCH)

**Type**: Byte size

**Default**: 10MB

**PHP Coordination**: Should be >= PHP `post_max_size` + 20%

**Examples**:
```bash
export HIGHPER_MAX_REQUEST_BODY=20MB
export HIGHPER_MAX_REQUEST_BODY=100MB  # For file upload apps
```

---

### HIGHPER_MAX_UPLOAD_SIZE

**Description**: Maximum individual file upload size

**Type**: Byte size

**Default**: Same as `max_request_body`

**PHP Coordination**: Should be >= PHP `upload_max_filesize` + 20%

**Examples**:
```bash
export HIGHPER_MAX_UPLOAD_SIZE=50MB
export HIGHPER_MAX_UPLOAD_SIZE=5MB
```

---

### HIGHPER_MAX_PATH_DEPTH

**Description**: Maximum directory path depth

**Type**: Integer

**Default**: 32

**Examples**:
```bash
export HIGHPER_MAX_PATH_DEPTH=64
export HIGHPER_MAX_PATH_DEPTH=16  # Stricter for security
```

---

### HIGHPER_MAX_CONNECTIONS_PER_IP

**Description**: Maximum concurrent connections from a single IP address

**Type**: Integer

**Default**: 100

**Examples**:
```bash
export HIGHPER_MAX_CONNECTIONS_PER_IP=500  # High-traffic API
export HIGHPER_MAX_CONNECTIONS_PER_IP=50   # Stricter limit
```

---

### HIGHPER_MAX_REQUESTS_PER_SECOND

**Description**: Maximum requests per second per IP (rate limiting)

**Type**: Integer

**Default**: 100

**Examples**:
```bash
export HIGHPER_MAX_REQUESTS_PER_SECOND=200
export HIGHPER_MAX_REQUESTS_PER_SECOND=1000  # CDN backend
```

---

## Global Configuration

### HIGHPER_LOG_LEVEL

**Description**: Logging verbosity level

**Type**: String (trace, debug, info, warn, error)

**Default**: info

**Examples**:
```bash
export HIGHPER_LOG_LEVEL=debug  # Development
export HIGHPER_LOG_LEVEL=warn   # Production
export HIGHPER_LOG_LEVEL=trace  # Troubleshooting
```

**Alternative**: You can also use `RUST_LOG` environment variable

---

### HIGHPER_METRICS_PORT

**Description**: Prometheus metrics endpoint port

**Type**: Integer

**Default**: 9090

**Examples**:
```bash
export HIGHPER_METRICS_PORT=9091
export HIGHPER_METRICS_PORT=8080
```

---

### HIGHPER_BIND_ADDRESS

**Description**: Server bind address

**Type**: String

**Default**: Configured in DSL

**Examples**:
```bash
export HIGHPER_BIND_ADDRESS=0.0.0.0:8080
export HIGHPER_BIND_ADDRESS=127.0.0.1:8080
```

---

## Usage Examples

### Development Environment

```bash
# Relaxed limits for development
export HIGHPER_LOG_LEVEL=debug
export HIGHPER_MAX_CONNECTIONS_PER_IP=10
export HIGHPER_MAX_REQUESTS_PER_SECOND=10
export HIGHPER_MAX_REQUEST_BODY=5MB

./highper-gateway --config dev.dsl
```

### Production Environment

```bash
# Production-ready limits
export HIGHPER_LOG_LEVEL=warn
export HIGHPER_MAX_CONNECTIONS_PER_IP=500
export HIGHPER_MAX_REQUESTS_PER_SECOND=1000
export HIGHPER_MAX_REQUEST_BODY=50MB
export HIGHPER_MAX_UPLOAD_SIZE=25MB
export HIGHPER_METRICS_PORT=9090

./highper-gateway --config prod.dsl
```

### Docker Container

```dockerfile
FROM ubuntu:22.04

# Install Highper Gateway
COPY highper-gateway /usr/local/bin/
COPY config.dsl /etc/highper/config.dsl

# Set environment-specific configuration
ENV HIGHPER_LOG_LEVEL=info
ENV HIGHPER_MAX_CONNECTIONS_PER_IP=200
ENV HIGHPER_MAX_REQUEST_BODY=20MB
ENV HIGHPER_METRICS_PORT=9090

EXPOSE 8080 9090

CMD ["highper-gateway", "--config", "/etc/highper/config.dsl"]
```

### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: highper-gateway
spec:
  replicas: 3
  template:
    spec:
      containers:
      - name: highper
        image: highper-gateway:latest
        env:
        - name: HIGHPER_LOG_LEVEL
          value: "info"
        - name: HIGHPER_MAX_CONNECTIONS_PER_IP
          value: "500"
        - name: HIGHPER_MAX_REQUESTS_PER_SECOND
          value: "1000"
        - name: HIGHPER_MAX_REQUEST_BODY
          value: "50MB"
        - name: HIGHPER_MAX_UPLOAD_SIZE
          value: "25MB"
        - name: HIGHPER_METRICS_PORT
          value: "9090"
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 9090
          name: metrics
        volumeMounts:
        - name: config
          mountPath: /etc/highper
      volumes:
      - name: config
        configMap:
          name: highper-config
```

### Docker Compose

```yaml
version: '3.8'

services:
  highper:
    image: highper-gateway:latest
    ports:
      - "8080:8080"
      - "9090:9090"
    environment:
      - HIGHPER_LOG_LEVEL=info
      - HIGHPER_MAX_CONNECTIONS_PER_IP=200
      - HIGHPER_MAX_REQUESTS_PER_SECOND=500
      - HIGHPER_MAX_REQUEST_BODY=30MB
      - HIGHPER_MAX_UPLOAD_SIZE=15MB
    volumes:
      - ./config.dsl:/etc/highper/config.dsl:ro
    restart: unless-stopped
```

---

## Environment-Specific Configuration

### Development (.env.development)

```bash
HIGHPER_LOG_LEVEL=debug
HIGHPER_MAX_CONNECTIONS_PER_IP=10
HIGHPER_MAX_REQUESTS_PER_SECOND=20
HIGHPER_MAX_REQUEST_BODY=10MB
HIGHPER_MAX_UPLOAD_SIZE=5MB
```

### Staging (.env.staging)

```bash
HIGHPER_LOG_LEVEL=info
HIGHPER_MAX_CONNECTIONS_PER_IP=100
HIGHPER_MAX_REQUESTS_PER_SECOND=200
HIGHPER_MAX_REQUEST_BODY=30MB
HIGHPER_MAX_UPLOAD_SIZE=15MB
```

### Production (.env.production)

```bash
HIGHPER_LOG_LEVEL=warn
HIGHPER_MAX_CONNECTIONS_PER_IP=500
HIGHPER_MAX_REQUESTS_PER_SECOND=1000
HIGHPER_MAX_REQUEST_BODY=50MB
HIGHPER_MAX_UPLOAD_SIZE=25MB
HIGHPER_METRICS_PORT=9090
```

**Usage with dotenv**:
```bash
# Load environment from file
source .env.production

# Start server
./highper-gateway --config prod.dsl
```

---

## PHP Application Coordination

When running PHP applications, coordinate environment variables with `php.ini` settings:

### Small WordPress Site

```bash
# php.ini settings:
# post_max_size = 64M
# upload_max_filesize = 32M
# max_execution_time = 60

# Highper environment (add 25% buffer):
export HIGHPER_MAX_REQUEST_BODY=80MB
export HIGHPER_MAX_UPLOAD_SIZE=40MB
export HIGHPER_MAX_CONNECTIONS_PER_IP=150
```

### Large File Upload Application

```bash
# php.ini settings:
# post_max_size = 200M
# upload_max_filesize = 100M
# max_execution_time = 300

# Highper environment (add 20% buffer):
export HIGHPER_MAX_REQUEST_BODY=240MB
export HIGHPER_MAX_UPLOAD_SIZE=120MB
export HIGHPER_MAX_CONNECTIONS_PER_IP=200
```

### High-Traffic API

```bash
# php.ini settings:
# post_max_size = 2M
# upload_max_filesize = 1M
# max_execution_time = 10

# Highper environment:
export HIGHPER_MAX_REQUEST_BODY=3MB
export HIGHPER_MAX_UPLOAD_SIZE=2MB
export HIGHPER_MAX_CONNECTIONS_PER_IP=1000
export HIGHPER_MAX_REQUESTS_PER_SECOND=2000
```

---

## Byte Size Format

All byte size environment variables support these suffixes:

| Suffix | Multiplier | Example | Bytes |
|--------|-----------|---------|-------|
| B | 1 | 100B | 100 |
| KB | 1,024 | 10KB | 10,240 |
| MB | 1,048,576 | 5MB | 5,242,880 |
| GB | 1,073,741,824 | 1GB | 1,073,741,824 |
| (none) | 1 | 50 | 50 |

**Examples**:
```bash
export HIGHPER_MAX_FILE_SIZE=100MB
export HIGHPER_MAX_REQUEST_BODY=10485760  # 10MB in bytes
export HIGHPER_MAX_UPLOAD_SIZE=5GB
```

---

## Duration Format

Duration environment variables support these suffixes:

| Suffix | Unit | Example | Duration |
|--------|------|---------|----------|
| ms | Milliseconds | 100ms | 100 milliseconds |
| s | Seconds | 30s | 30 seconds |
| m | Minutes | 5m | 5 minutes |
| h | Hours | 1h | 1 hour |
| d | Days | 2d | 2 days |

**Note**: Currently used for internal timeouts. Future versions may expose more timeout settings.

---

## Boolean Format

Boolean environment variables accept these values:

**True**: `true`, `1`, `yes`, `on` (case-insensitive)

**False**: `false`, `0`, `no`, `off` (case-insensitive)

**Example**:
```bash
export HIGHPER_ENABLE_FEATURE=true
export HIGHPER_ENABLE_FEATURE=1
export HIGHPER_ENABLE_FEATURE=yes
```

---

## Validation

### Check Applied Configuration

Currently, environment variable overrides are applied at runtime. To verify:

1. **Start with debug logging**:
   ```bash
   export HIGHPER_LOG_LEVEL=debug
   ./highper-gateway --config config.dsl
   ```

2. **Check logs** for applied settings

3. **Test limits** by sending requests that exceed/meet the limits

### Future Enhancement

A CLI command for validating configuration will be added:

```bash
# Future feature
highper-gateway --validate-config config.dsl
```

This will show:
- DSL configuration values
- Environment variable overrides
- Final applied configuration

---

## Troubleshooting

### Issue: Environment Variable Not Applied

**Symptom**: Changes to environment variables don't take effect

**Solution**:
1. Verify variable name spelling (must start with `HIGHPER_`)
2. Check variable value format (e.g., `10MB` not `10 MB`)
3. Restart the server after changing environment variables
4. Check for DSL configuration that might override

### Issue: Invalid Value Format

**Symptom**: Server fails to start or ignores environment variable

**Solution**:
1. Check byte size format: `10MB` ✅, `10 MB` ❌, `10m` ❌
2. Check boolean format: `true` ✅, `True` ✅, `t` ❌
3. Check integer format: `100` ✅, `100.5` ❌, `hundred` ❌

### Issue: PHP Still Rejects Uploads

**Symptom**: Highper accepts but PHP rejects

**Solution**:
1. Verify PHP settings: `php -i | grep post_max_size`
2. Ensure Highper limits >= PHP limits + buffer
3. Restart PHP-FPM after changing php.ini
4. Check both `post_max_size` and `upload_max_filesize`

---

## Best Practices

### 1. Use .env Files for Local Development

```bash
# .env.local
HIGHPER_LOG_LEVEL=debug
HIGHPER_MAX_CONNECTIONS_PER_IP=10

# Load and run
source .env.local
./highper-gateway --config dev.dsl
```

### 2. Use Secret Management in Production

Don't store sensitive environment variables in version control:

```bash
# ❌ Bad: Committed to git
echo "HIGHPER_API_KEY=secret" >> .env

# ✅ Good: Use secret management
kubectl create secret generic highper-secrets \
  --from-literal=api-key=secret
```

### 3. Document Your Overrides

Include a comment block in deployment configs:

```yaml
# Environment variable overrides
# Base config: config.dsl
# Overrides for production high-traffic scenario
env:
  - name: HIGHPER_MAX_CONNECTIONS_PER_IP
    value: "500"  # Increased from default 100
```

### 4. Test Environment Variables

Create a test script:

```bash
#!/bin/bash
# test-env-config.sh

export HIGHPER_LOG_LEVEL=debug
export HIGHPER_MAX_REQUEST_BODY=1MB

# Should reject 2MB upload
curl -X POST -d @2mb-file.json http://localhost:8080/api/test

# Should accept 512KB upload
curl -X POST -d @512kb-file.json http://localhost:8080/api/test
```

---

## Summary

### Quick Reference

| Environment Variable | Type | Default | Description |
|---------------------|------|---------|-------------|
| HIGHPER_MAX_FILE_SIZE | Byte size | 100MB | Maximum static file size |
| HIGHPER_MAX_REQUEST_BODY | Byte size | 10MB | Maximum request body size |
| HIGHPER_MAX_UPLOAD_SIZE | Byte size | 10MB | Maximum upload file size |
| HIGHPER_MAX_PATH_DEPTH | Integer | 32 | Maximum path depth |
| HIGHPER_MAX_CONNECTIONS_PER_IP | Integer | 100 | Max connections per IP |
| HIGHPER_MAX_REQUESTS_PER_SECOND | Integer | 100 | Max requests/sec per IP |
| HIGHPER_LOG_LEVEL | String | info | Log level (trace/debug/info/warn/error) |
| HIGHPER_METRICS_PORT | Integer | 9090 | Prometheus metrics port |

### Configuration Hierarchy

```
Environment Variables (HIGHPER_*)
    ↓ (overrides)
DSL Configuration File
    ↓ (overrides)
Built-in Defaults
```

### 12-Factor Compliance

✅ **Factor III: Config** - Store config in the environment

Environment variables provide clean separation of code and configuration,
enabling the same codebase to be deployed across multiple environments
with different settings.

---

**Environment Configuration Guide Complete** ✅

**See Also**:
- [PHP Compatibility Guide](./PHP_COMPATIBILITY_GUIDE.md) - Coordinate with PHP settings
- [Production Hardening](./PRODUCTION_HARDENING.md) - Security and limits
- [Validation Report](./VALIDATION_REPORT.md) - 12-factor compliance
