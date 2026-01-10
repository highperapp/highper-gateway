# PHP-FPM Demo

This demo shows PHP-FPM and static file serving with Highper Gateway.

---

## 🚀 Quick Start

### Option 1: Local PHP-FPM (Recommended)

**Prerequisites**:
- PHP 8.x with FPM installed
- Highper Gateway built

**Steps**:

1. **Install PHP-FPM**:
   ```bash
   # Ubuntu/Debian
   sudo apt install php8.2-fpm

   # macOS
   brew install php
   ```

2. **Start PHP-FPM**:
   ```bash
   # Ubuntu/Debian
   sudo systemctl start php8.2-fpm
   sudo systemctl status php8.2-fpm

   # macOS
   brew services start php
   ```

3. **Verify socket**:
   ```bash
   # Ubuntu/Debian
   ls -la /var/run/php/php8.2-fpm.sock

   # macOS
   ls -la /usr/local/var/run/php-fpm.sock
   ```

4. **Update demo.dsl** (if needed):
   Edit `demo.dsl` and update the socket path to match your system.

5. **Run Highper Gateway**:
   ```bash
   # From repository root
   cargo build --release
   ./target/release/highper-gateway --config demo/php-fpm/demo.dsl
   ```

6. **Test**:
   ```bash
   # Open in browser
   open http://localhost:8080

   # Or use curl
   curl http://localhost:8080
   curl http://localhost:8080/info.php
   curl http://localhost:8080/static/test.html

   # Test POST
   curl -X POST -d "test=data" http://localhost:8080/test-post.php
   ```

---

### Option 2: Docker Compose

**Prerequisites**:
- Docker and Docker Compose

**Steps**:

1. **Start services**:
   ```bash
   cd demo/php-fpm
   docker-compose up -d
   ```

2. **Check status**:
   ```bash
   docker-compose ps
   docker-compose logs php-fpm
   ```

3. **Update configuration**:
   Edit `demo.dsl` to use TCP socket:
   ```dsl
   php_fpm enabled socket="php-fpm:9000" pool_size=50
   ```

4. **Run gateway** (outside container):
   ```bash
   cargo run -- --config demo/php-fpm/demo.dsl
   ```

5. **Test**:
   ```bash
   curl http://localhost:8080
   ```

6. **Cleanup**:
   ```bash
   docker-compose down
   ```

---

## 📁 Demo Files

```
demo/php-fpm/
├── index.php          # Main test page with server info
├── info.php           # Standard phpinfo() output
├── test-post.php      # POST request handler test
├── static/
│   └── test.html      # Static file serving test
├── demo.dsl           # Highper Gateway configuration
├── docker-compose.yml # Docker setup
└── README.md          # This file
```

---

## 🧪 Testing Features

### 1. PHP Processing

Test that PHP scripts are executed via FastCGI:

```bash
# Home page (index.php)
curl http://localhost:8080/

# PHP info page
curl http://localhost:8080/info.php

# Should return HTML with PHP version info
```

### 2. Static File Serving

Test that static files are served directly:

```bash
# Static HTML file
curl http://localhost:8080/static/test.html

# Should return static HTML without PHP processing
```

### 3. Index Files

Test directory index resolution:

```bash
# Should serve index.php
curl http://localhost:8080/

# Verify index.php is served
curl -I http://localhost:8080/
```

### 4. try_files Pattern

Test Nginx-style fallback:

```bash
# Existing file
curl http://localhost:8080/index.php

# Non-existing file (falls back to index.php)
curl http://localhost:8080/nonexistent

# Static file with 404 on missing
curl http://localhost:8080/static/missing.txt
# Should return 404
```

### 5. POST Requests

Test POST body handling:

```bash
# JSON POST
curl -X POST \
  -H "Content-Type: application/json" \
  -d '{"test": "data"}' \
  http://localhost:8080/test-post.php

# Form POST
curl -X POST \
  -d "name=value&other=data" \
  http://localhost:8080/test-post.php
```

### 6. Performance

Benchmark static vs PHP:

```bash
# Static file (should be very fast)
ab -n 10000 -c 100 http://localhost:8080/static/test.html

# PHP file (depends on PHP-FPM performance)
ab -n 1000 -c 50 http://localhost:8080/index.php
```

### 7. Custom Error Pages

Test custom error page functionality:

```bash
# Test 404 error page
curl http://localhost:8080/nonexistent-file

# Should return beautiful styled 404.html page

# Test 500 error page (if PHP-FPM misconfigured)
# Will show custom 500.html page
```

### 8. Range Requests (206 Partial Content)

Test range request support for video/audio streaming:

```bash
# Request first 1KB of a file
curl -H "Range: bytes=0-1023" http://localhost:8080/static/test.html

# Request from offset to end
curl -H "Range: bytes=1024-" http://localhost:8080/static/test.html

# Request last 500 bytes
curl -H "Range: bytes=-500" http://localhost:8080/static/test.html

# Should return 206 Partial Content with Content-Range header
```

### 9. Conditional Requests (Caching)

Test ETag and If-Modified-Since headers:

```bash
# Get ETag
ETAG=$(curl -I http://localhost:8080/static/test.html | grep -i etag | cut -d' ' -f2)

# Use ETag for conditional request
curl -H "If-None-Match: $ETAG" http://localhost:8080/static/test.html
# Should return 304 Not Modified

# Use If-Modified-Since
curl -H "If-Modified-Since: $(date -R)" http://localhost:8080/static/test.html
# Should return 304 Not Modified if file hasn't changed
```

### 10. Directory Listing

Test automatic directory indexing:

```bash
# Access directory without index file
# (Create a test directory without index.php/index.html)
mkdir -p demo/php-fpm/testdir
touch demo/php-fpm/testdir/file1.txt
touch demo/php-fpm/testdir/file2.html

# Visit in browser or curl
curl http://localhost:8080/testdir/

# Should return beautiful HTML directory listing with:
# - File and directory icons
# - File sizes in human-readable format
# - Last modified timestamps
# - Parent directory link
```

---

## 🔧 Configuration Explained

```dsl
http://localhost:8080 {
    # Document root for all files
    root "demo/php-fpm"

    # Try these files when accessing a directory
    index index.php index.html

    # Custom error pages (NEW!)
    error_page 404 "/404.html"
    error_page 500 "/500.html"

    # Enable directory listing for directories without index files (NEW!)
    directory_listing on

    # Static files in /static/* are served directly
    /static/* {
        static_files
        try_files $uri =404  # Return 404 if not found
    }

    # PHP files go through FastCGI
    /*.php {
        php_fpm enabled socket="/var/run/php/php-fpm.sock" pool_size=50
        read_timeout=60s
        proxy localhost:9000
    }

    # Everything else tries file, then directory, then index.php
    /* {
        try_files $uri $uri/ /index.php
    }
}
```

**Key Points**:
- `root`: Base directory for file resolution
- `index`: Files to try when accessing a directory
- `error_page`: Custom error pages for specific status codes (NEW!)
- `directory_listing`: Enable automatic directory indexing (NEW!)
- `static_files`: Enable static file serving (no PHP)
- `try_files`: Nginx-style fallback patterns
- `php_fpm`: FastCGI configuration
  - `socket`: Unix socket or TCP address
  - `pool_size`: Connection pool size
  - `read_timeout`: Timeout for PHP execution

**Enhancement Features** (automatically enabled):
- **Conditional Requests**: ETag and If-Modified-Since headers for efficient caching
- **Range Requests**: 206 Partial Content support for video/audio streaming
- **Custom Error Pages**: Beautiful HTML error pages instead of plain text
- **Directory Listing**: Automatic HTML index generation for directories

---

## 📊 Expected Behavior

### Request: `GET /`
1. Resolve to `demo/php-fpm/` (directory)
2. Try index files: `index.php`, `index.html`
3. Find `index.php`
4. Route to PHP-FPM
5. Execute and return response

### Request: `GET /static/test.html`
1. Resolve to `demo/php-fpm/static/test.html`
2. File exists
3. Serve via static file handler (sendfile)
4. Return with proper MIME type

### Request: `GET /nonexistent`
1. Try `demo/php-fpm/nonexistent` (not found)
2. Try `demo/php-fpm/nonexistent/` (not a directory)
3. Fall back to `/index.php`
4. Route to PHP-FPM
5. Execute index.php

---

## 🐛 Troubleshooting

### PHP-FPM Socket Not Found

**Error**: "Failed to connect to /var/run/php/php-fpm.sock"

**Solutions**:
1. Check PHP-FPM is running:
   ```bash
   ps aux | grep php-fpm
   sudo systemctl status php8.2-fpm
   ```

2. Find correct socket path:
   ```bash
   # Ubuntu/Debian
   ls /var/run/php/

   # macOS
   ls /usr/local/var/run/
   ```

3. Update `demo.dsl` with correct path

4. Check permissions:
   ```bash
   ls -la /var/run/php/php-fpm.sock
   ```

### Permission Denied

**Error**: "Permission denied" when accessing socket

**Solution**:
```bash
# Temporarily for testing (not for production!)
sudo chmod 777 /var/run/php/php-fpm.sock

# Or run gateway as same user as PHP-FPM
sudo -u www-data ./target/release/highper-gateway --config demo/php-fpm/demo.dsl
```

### File Not Found (404)

**Error**: 404 for existing PHP files

**Solutions**:
1. Check root path in config:
   ```bash
   ls demo/php-fpm/index.php
   ```

2. Verify file permissions:
   ```bash
   chmod 644 demo/php-fpm/*.php
   ```

3. Check logs:
   ```bash
   # Gateway logs show file resolution
   ```

---

## 📈 Performance Tips

1. **Use Unix Sockets** (faster than TCP):
   ```dsl
   socket="/var/run/php/php-fpm.sock"
   ```

2. **Tune Pool Size** based on traffic:
   ```dsl
   pool_size=100  # For high traffic
   ```

3. **Adjust PHP-FPM Workers**:
   Edit `/etc/php/8.2/fpm/pool.d/www.conf`:
   ```ini
   pm = dynamic
   pm.max_children = 50
   pm.start_servers = 10
   pm.min_spare_servers = 5
   pm.max_spare_servers = 20
   ```

4. **Enable OpCache** in php.ini:
   ```ini
   opcache.enable=1
   opcache.memory_consumption=128
   opcache.max_accelerated_files=10000
   ```

---

## 🎯 Next Steps

1. **WordPress**: Try running WordPress with this setup
2. **Laravel**: Deploy a Laravel application
3. **Load Test**: Benchmark with `wrk` or `ab`
4. **Production**: Add TLS, logging, monitoring

---

## 📚 See Also

- [PHP-FPM Quick Start Guide](../../PHP_FPM_QUICK_START.md)
- [Complete Implementation Guide](../../PHP_FPM_DSL_COMPLETE.md)
- [Feature Documentation](../../PHP_FPM_FEATURE_COMPLETE.md)

---

**Happy Testing!** 🚀
