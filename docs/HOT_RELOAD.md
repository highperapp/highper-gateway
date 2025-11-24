# Hot Reload Configuration

**Status:** ✅ Implemented in Phase 1.2
**Version:** 0.1.0+

## Overview

Highper Gateway supports hot reload of configuration without requiring a restart or dropping active connections. Configuration changes are automatically detected, validated, and applied at runtime with zero downtime.

## Features

- ✅ **Automatic Reload** - File changes detected via filesystem notifications
- ✅ **Manual Reload** - Trigger reload via SIGHUP signal
- ✅ **Zero Downtime** - Active connections maintained during reload
- ✅ **Validation** - New configuration validated before applying
- ✅ **Rollback** - Invalid configurations rejected, old config retained
- ✅ **Cross-Platform** - Works on Linux (inotify), macOS (FSEvents), Windows (polling)

## Usage

### Enable Hot Reload (Default)

Hot reload is enabled by default when starting the proxy:

```bash
./highper-gateway -c config.yaml
```

You'll see confirmation messages:

```
Hot reload enabled - configuration changes will be applied automatically
Send SIGHUP signal to manually reload configuration
Started watching config file: "config.yaml"
```

### Disable Hot Reload

To run without hot reload (requires restart for config changes):

```bash
./highper-gateway -c config.yaml --hot-reload=false
```

### Manual Reload via SIGHUP

You can manually trigger a reload at any time using the SIGHUP signal:

```bash
# Find the proxy process ID
pidof highper-gateway

# Send SIGHUP signal
kill -HUP $(pidof highper-gateway)
```

Or use:

```bash
pkill -HUP highper-gateway
```

The proxy will reload the configuration and log:

```
Received SIGHUP, triggering configuration reload
Configuration file changed, reloading...
Configuration reloaded successfully
```

## What Can Be Reloaded?

Hot reload supports changes to most configuration sections:

### ✅ Fully Supported (Zero Downtime)

- **Routes** - Add, remove, or modify routes
- **Upstreams** - Update backend servers and weights
- **Middleware** - Enable/disable middleware
- **Rate Limits** - Adjust rate limiting rules
- **Timeouts** - Change timeout values
- **Retry Policies** - Update retry configuration
- **Health Checks** - Modify health check settings
- **Transformations** - Update request/response transforms
- **Observability** - Change logging and metrics settings

### ⚠️ Partially Supported

- **TLS Configuration** - TLS changes require connection drain
  - New certificates loaded automatically
  - Protocol changes may require reconnection

### ❌ Not Reloadable (Requires Restart)

- **Server Bind Addresses** - Cannot change listening ports at runtime
- **Worker Count** - Thread pool size set at startup
- **Performance Settings** - Low-level runtime settings fixed at startup

## How It Works

### Architecture

```
Configuration File Change
        │
        ↓
  File Watcher (inotify/FSEvents)
        │
        ↓
  Read & Parse New Config
        │
        ↓
  Validate Configuration
        │
        ├──Invalid──→ Log Error + Keep Old Config
        │
        ↓ Valid
  Apply Changes Atomically
        │
        ├────→ Update Routes
        ├────→ Update Upstreams
        ├────→ Update Middleware
        └────→ Update Settings
```

### Validation Process

Before applying any changes, the reloader:

1. **Parses** the new configuration file
2. **Validates** syntax and structure
3. **Checks** for required fields
4. **Verifies** upstream servers are reachable (optional)
5. **Computes** diff from current configuration

If validation fails at any step, the old configuration is retained and an error is logged.

### Atomic Updates

Configuration updates use `Arc<RwLock<Config>>` to ensure:

- **Thread Safety** - Multiple workers can read config concurrently
- **Consistency** - All workers see the same config version
- **Atomicity** - Updates are all-or-nothing

## Examples

### Example 1: Adding a New Route

Original `config.yaml`:

```yaml
routes:
  - path: "/api"
    upstream: backend
```

Modified `config.yaml`:

```yaml
routes:
  - path: "/api"
    upstream: backend
  - path: "/admin"    # NEW
    upstream: admin_backend  # NEW
```

After save, the proxy logs:

```
Configuration file changed, reloading...
Configuration reloaded successfully
Routes changed: 1 → 2
```

The new `/admin` route is immediately available without restart.

### Example 2: Updating Upstream Servers

Original:

```yaml
upstreams:
  - name: backend
    servers:
      - address: "10.0.1.10:8080"
      - address: "10.0.1.11:8080"
```

Modified (adding a server):

```yaml
upstreams:
  - name: backend
    servers:
      - address: "10.0.1.10:8080"
      - address: "10.0.1.11:8080"
      - address: "10.0.1.12:8080"  # NEW
```

Result:

```
Configuration reloaded successfully
Upstreams changed: 2 servers → 3 servers
```

Traffic immediately starts flowing to the new backend server.

### Example 3: Changing Timeouts

Original:

```yaml
routes:
  - path: "/api"
    upstream: backend
    timeout: 30s
```

Modified:

```yaml
routes:
  - path: "/api"
    upstream: backend
    timeout: 60s  # CHANGED
```

New requests use the 60s timeout immediately.

## Error Handling

### Invalid Configuration

If you save an invalid configuration:

```yaml
routes:
  - path: "/api"
    upstream: nonexistent_upstream  # ERROR: upstream doesn't exist
```

The proxy logs:

```
Configuration file changed, reloading...
Configuration validation failed: Upstream 'nonexistent_upstream' not found
```

The old configuration remains active, and the proxy continues operating normally.

### File System Errors

If the configuration file is deleted:

```
Configuration file deleted - keeping current configuration
```

The proxy continues running with the last valid configuration.

## Monitoring Reloads

### Log Messages

Watch for reload events in logs:

```bash
# Follow logs for reload events
tail -f /var/log/highper-gateway.log | grep -i reload
```

Expected messages:

- `Configuration file changed, reloading...` - Reload triggered
- `Configuration reloaded successfully` - Reload completed
- `Configuration validation failed: <error>` - Reload failed

### Metrics

Reload metrics are exposed at the `/metrics` endpoint:

```
# Configuration reload attempts
config_reload_total{status="success"} 15
config_reload_total{status="failure"} 2

# Last reload timestamp
config_reload_timestamp 1698765432

# Current configuration version
config_version_info{file="config.yaml"} 1
```

## Best Practices

### 1. Test Configuration Before Applying

Use the `--validate` flag to test configuration without starting the proxy:

```bash
./highper-gateway -c new-config.yaml --validate
```

### 2. Use Version Control

Keep configuration in git to track changes and enable rollback:

```bash
git add config.yaml
git commit -m "Add new admin route"
```

### 3. Atomic File Updates

Editors like vim save atomically by default. If using scripts:

```bash
# Write to temp file, then move (atomic)
cat > config.yaml.tmp << EOF
server:
  bind: ["0.0.0.0:8080"]
EOF
mv config.yaml.tmp config.yaml
```

### 4. Monitor Reload Success

Set up alerting for failed reloads:

```yaml
# Prometheus alert
- alert: ConfigReloadFailure
  expr: increase(config_reload_total{status="failure"}[5m]) > 0
  annotations:
    summary: "Configuration reload failed"
```

### 5. Gradual Rollout

For large changes, update configuration in stages:

1. Add new routes
2. Test new routes
3. Remove old routes

## Troubleshooting

### Reload Not Triggering

**Problem:** Config file changed but reload doesn't trigger

**Solutions:**

1. Check file watcher is active:
   ```
   Started watching config file: "config.yaml"
   ```

2. Ensure file is being saved atomically (editors like vim do this automatically)

3. Check file permissions:
   ```bash
   ls -la config.yaml
   ```

4. Try manual reload:
   ```bash
   kill -HUP $(pidof highper-gateway)
   ```

### Validation Errors

**Problem:** Config fails validation

**Solutions:**

1. Check syntax:
   ```bash
   yamllint config.yaml
   ```

2. Validate manually:
   ```bash
   ./highper-gateway -c config.yaml --validate
   ```

3. Check logs for specific error:
   ```
   Configuration validation failed: <detailed error>
   ```

### Performance Impact

**Problem:** Worried about reload performance

**Answer:**
- Reloads are fast (< 100ms for typical configs)
- No impact on active requests
- Config reads are lock-free (RwLock)
- Updates are atomic

## Technical Details

### File Watching

- **Linux:** Uses inotify (native kernel notifications)
- **macOS:** Uses FSEvents (native filesystem events)
- **Windows:** Uses polling (checks every 2 seconds)

### Thread Safety

Configuration is stored in `Arc<RwLock<Config>>`:

- **Arc:** Shared ownership across threads
- **RwLock:** Multiple readers, single writer
- **Read operations:** Lock-free for readers
- **Write operations:** Brief write lock during update

### Reload Latency

Typical reload times:

- **File detection:** < 10ms (inotify/FSEvents) or ~2s (Windows)
- **Parse + validate:** 10-50ms (depends on config size)
- **Apply changes:** < 10ms
- **Total:** Usually < 100ms

## See Also

- [Configuration Reference](CONFIGURATION.md)
- [Signal Handling](SIGNALS.md)
- [Deployment Guide](DEPLOYMENT.md)
- [Monitoring](MONITORING.md)

## FAQ

**Q: Is hot reload production-ready?**
A: Yes, it's tested and designed for zero-downtime operation.

**Q: What happens to in-flight requests during reload?**
A: They continue using the old configuration until completion.

**Q: Can I reload TLS certificates?**
A: Yes, new certificates are loaded automatically (see Certificate Hot Reload).

**Q: Does hot reload work in Docker?**
A: Yes, mount the config file and edit it from the host.

**Q: Can I disable hot reload?**
A: Yes, use `--hot-reload=false` flag.

---

**Hot Reload Status:** ✅ Production Ready
**Last Updated:** October 30, 2025
