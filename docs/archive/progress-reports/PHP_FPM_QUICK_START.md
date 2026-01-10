# PHP-FPM DSL Quick Start Guide

This guide shows you how to quickly configure PHP-FPM and static file serving using the highper-gateway DSL.

---

## Basic PHP Application

The simplest PHP-FPM configuration:

```dsl
http://localhost:8080 {
    root "/var/www/html"
    index index.php index.html

    /*.php {
        php_fpm enabled socket="/var/run/php/php-fpm.sock"
        proxy localhost:9000
    }

    /* {
        try_files $uri $uri/ /index.php
    }
}
```

**What this does**:
- Serves files from `/var/www/html`
- Routes `*.php` files to PHP-FPM via Unix socket
- Falls back to `index.php` for non-existent files

---

## WordPress Site

Complete WordPress configuration with static asset optimization:

```dsl
https://blog.example.com {
    root "/var/www/wordpress"
    index index.php

    # Serve wp-content directly (no PHP processing)
    /wp-content/* {
        static_files
        try_files $uri =404
    }

    # PHP processing for all .php files
    /*.php {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=100
        proxy localhost:9000
    }

    # Pretty permalinks
    /* {
        try_files $uri $uri/ /index.php
    }

    tls admin@example.com
    cors origins="https://blog.example.com"
}
```

**Key Features**:
- Auto HTTPS with Let's Encrypt
- Static files served directly (faster)
- PHP-FPM pool of 100 connections
- CORS enabled for your domain

---

## Laravel Application

Laravel with API rate limiting:

```dsl
https://api.example.com {
    root "/var/www/laravel/public"
    index index.php

    # Static assets
    /css/* {
        static_files
    }

    /js/* {
        static_files
    }

    # API with rate limiting
    /api/* {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=200 read_timeout=60s
        rate_limit 1000 burst=100 per 1m per_ip
        proxy localhost:9000
    }

    # Laravel routing
    /* {
        try_files $uri $uri/ /index.php
    }

    tls internal
    compress gzip br
}
```

**Key Features**:
- Laravel public directory as root
- API rate limiting (1000 req/min per IP)
- Compression enabled
- Self-signed TLS for development

---

## Static Site with PHP Contact Form

Mostly static content with one PHP endpoint:

```dsl
http://localhost:8080 {
    root "/var/www/site"
    index index.html

    # Single PHP endpoint
    /contact.php {
        php_fpm enabled socket="/var/run/php/php-fpm.sock"
        rate_limit 10 per 1m per_ip
        proxy localhost:9000
    }

    # Everything else is static
    /* {
        static_files
        try_files $uri $uri/ /index.html
    }
}
```

**Key Features**:
- Static HTML/CSS/JS served directly
- Single PHP endpoint for contact form
- Rate limiting on contact form (prevent spam)

---

## Configuration Options

### PHP-FPM Options

```dsl
php_fpm enabled                                    # Enable PHP-FPM
php_fpm socket="/var/run/php/php-fpm.sock"        # Unix socket (recommended)
php_fpm socket="127.0.0.1:9000"                   # TCP socket (alternative)
php_fpm pool_size=50                              # Connection pool size
php_fpm connect_timeout=5s                        # Connection timeout
php_fpm read_timeout=60s                          # Read timeout
php_fpm write_timeout=60s                         # Write timeout
php_fpm keepalive=90s                             # Keepalive duration
php_fpm script_extensions .php .phtml             # File extensions to process
```

### Static File Options

```dsl
root "/var/www/html"                              # Document root
index index.php index.html index.htm              # Index files (in order)
static_files                                      # Enable static file serving
try_files $uri $uri/ /index.php                   # File fallback pattern
```

### Try Files Patterns

```dsl
try_files $uri =404                               # Try file, then 404
try_files $uri $uri/ /index.html                  # Try file, directory, fallback
try_files $uri $uri/ /index.php                   # Laravel/WordPress style
try_files $uri =404                               # Simple static files
```

---

## Performance Tips

### 1. Use Unix Sockets (Faster)
```dsl
socket="/var/run/php/php-fpm.sock"  # ✅ Recommended
socket="127.0.0.1:9000"             # ⚠️  Slower (TCP overhead)
```

### 2. Tune Pool Size
```dsl
# Low traffic
pool_size=10

# Medium traffic (WordPress blog)
pool_size=50

# High traffic (API or large site)
pool_size=200

# Very high traffic
pool_size=500
```

### 3. Adjust Timeouts for Long-Running Scripts
```dsl
# Default (good for most cases)
read_timeout=60s

# Long-running admin operations
read_timeout=120s

# Background jobs or imports
read_timeout=300s
```

### 4. Enable Compression
```dsl
compress gzip br                    # Enable both gzip and brotli
```

### 5. Use Static File Directive for Assets
```dsl
/assets/* {
    static_files                    # Skip PHP-FPM entirely
    cache enabled ttl=1h            # Cache for 1 hour
}
```

---

## Common Patterns

### Pattern 1: PHP + Static Files
```dsl
/*.php {
    php_fpm enabled socket="/var/run/php/php-fpm.sock"
    proxy localhost:9000
}

/* {
    static_files
    try_files $uri $uri/ /index.php
}
```

### Pattern 2: Multi-Directory Setup
```dsl
/app/* {
    php_fpm enabled socket="/var/run/php/php8.2-fpm.sock"
    proxy localhost:9000
}

/admin/* {
    php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" read_timeout=120s
    proxy localhost:9000
}
```

### Pattern 3: Different PHP Versions
```dsl
/app-php82/* {
    php_fpm enabled socket="/var/run/php/php8.2-fpm.sock"
    proxy localhost:9000
}

/app-php81/* {
    php_fpm enabled socket="/var/run/php/php8.1-fpm.sock"
    proxy localhost:9001
}

/legacy/* {
    php_fpm enabled socket="/var/run/php/php7.4-fpm.sock"
    proxy localhost:9002
}
```

---

## Testing Your Configuration

### 1. Parse the DSL
```bash
# Your DSL file will be automatically parsed when you run highper-gateway
highper-gateway --config my-site.dsl
```

### 2. Check Generated YAML
The DSL converter creates a temporary YAML file. Check logs for the path.

### 3. Test PHP-FPM Connection
```bash
# Make sure PHP-FPM is running
sudo systemctl status php8.2-fpm

# Test the socket
ls -la /var/run/php/php-fpm.sock
```

### 4. Simple Test Request
```bash
# Create a test PHP file
echo "<?php phpinfo(); ?>" > /var/www/html/info.php

# Request it
curl http://localhost:8080/info.php
```

---

## Troubleshooting

### PHP-FPM Not Working

**Problem**: 502 Bad Gateway or Connection refused

**Solutions**:
1. Check PHP-FPM is running:
   ```bash
   sudo systemctl status php8.2-fpm
   ```

2. Verify socket path:
   ```bash
   ls -la /var/run/php/
   ```

3. Check permissions:
   ```bash
   sudo chmod 777 /var/run/php/php-fpm.sock  # For testing only!
   ```

4. Check PHP-FPM logs:
   ```bash
   sudo tail -f /var/log/php8.2-fpm.log
   ```

### Files Not Found (404)

**Problem**: Static files return 404

**Solutions**:
1. Verify root path:
   ```bash
   ls -la /var/www/html
   ```

2. Check file permissions:
   ```bash
   chmod -R 755 /var/www/html
   ```

3. Verify try_files pattern matches your setup

### Slow PHP Requests

**Problem**: PHP requests are slow

**Solutions**:
1. Increase pool size:
   ```dsl
   pool_size=200
   ```

2. Reduce timeouts if appropriate:
   ```dsl
   read_timeout=30s
   ```

3. Check PHP-FPM pool settings in `/etc/php/8.2/fpm/pool.d/www.conf`

---

## Next Steps

1. **See Full Examples**: Check `examples/php-fpm-scenarios.dsl` for 8 complete scenarios
2. **Read Full Documentation**: See `PHP_FPM_DSL_COMPLETE.md` for implementation details
3. **Check Status**: See `PHP_FPM_IMPLEMENTATION_STATUS.md` for runtime integration status

---

## Example Sites

Copy-paste these complete working examples:

### Simple Blog
```dsl
log info
metrics prometheus port=9090

http://blog.local:8080 {
    root "/var/www/blog"
    index index.php index.html

    /*.php {
        php_fpm enabled socket="/var/run/php/php-fpm.sock" pool_size=50
        proxy localhost:9000
    }

    /* {
        static_files
        try_files $uri $uri/ /index.php
    }
}
```

### E-Commerce Site
```dsl
log info
metrics prometheus port=9090

https://shop.example.com {
    root "/var/www/shop"
    index index.php

    # Product images (static, cached)
    /images/* {
        static_files
        cache enabled ttl=24h
    }

    # API endpoints (rate limited)
    /api/* {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=200
        rate_limit 5000 burst=500 per 1h per_ip
        proxy backend1:9000 backend2:9000
        lb round_robin
    }

    # All other PHP
    /*.php {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=200
        proxy backend1:9000 backend2:9000
        lb least_conn
    }

    # Routing
    /* {
        try_files $uri $uri/ /index.php
    }

    tls admin@example.com
    compress gzip br
    cors origins="https://shop.example.com"
}
```

---

**Quick Start Complete!** 🚀

For more advanced configurations, see the full documentation and examples.
