# Use Case 14: Static Files + FastCGI

Static file serving with FastCGI (PHP-FPM) backend for dynamic content.

## Overview

| Property | Value |
|----------|-------|
| Protocol | HTTP/HTTPS |
| Ports | 80, 443 |
| TLS Required | Optional |
| Privileged | Yes |
| Scaling | Vertical |

## When to Use

- PHP applications (WordPress, Laravel)
- Mixed static and dynamic content
- Legacy web applications
- FastCGI backends

## Architecture

```
                         ┌──────────────────────┐
    HTTP:443   ────────▶ │   Highper Gateway    │
                         │                      │
    *.css, *.js ────────▶│   Static Files       │────▶ /var/www/html
                         │                      │
    *.php      ────────▶│   FastCGI Proxy      │────▶ PHP-FPM
                         └──────────────────────┘
```

## Configuration

### Static Files

```yaml
static:
  root: /var/www/html
  index:
    - index.html
    - index.php
  directory_listing: false
```

### Route-Based Handling

```yaml
routes:
  # Static assets
  - match:
      path_regex: "\\.(css|js|jpg|png|gif|ico)$"
    static:
      root: /var/www/html
      cache_control: "public, max-age=31536000"

  # PHP files
  - match:
      path_regex: "\\.php$"
    fastcgi:
      backend: php-fpm
      script_filename: /var/www/html$uri

  # Default: try file, then PHP
  - match:
      path_prefix: /
    static:
      root: /var/www/html
      try_files:
        - $uri
        - $uri/
        - /index.php$is_args$args
```

### FastCGI Backend

```yaml
backends:
  - name: php-fpm
    protocol: fastcgi
    servers:
      - address: "unix:/var/run/php/php-fpm.sock"
      # Or TCP:
      # - address: "127.0.0.1:9000"
```

### FastCGI Parameters

```yaml
fastcgi:
  params:
    SCRIPT_FILENAME: /var/www/html$uri
    SCRIPT_NAME: $uri
    REQUEST_URI: $uri
    QUERY_STRING: $query_string
    REQUEST_METHOD: $method
    CONTENT_TYPE: $content_type
    CONTENT_LENGTH: $content_length
```

## Compression

```yaml
compression:
  enabled: true
  algorithms:
    - br
    - gzip
  types:
    - text/html
    - text/css
    - application/javascript
```

## Security Headers

```yaml
headers:
  response:
    add:
      - name: X-Content-Type-Options
        value: nosniff
      - name: X-Frame-Options
        value: SAMEORIGIN
    remove:
      - X-Powered-By
      - Server
```

## PHP-FPM Configuration

```ini
; /etc/php/8.1/fpm/pool.d/www.conf
[www]
listen = /var/run/php/php-fpm.sock
listen.owner = www-data
listen.group = www-data
pm = dynamic
pm.max_children = 50
pm.start_servers = 5
pm.min_spare_servers = 5
pm.max_spare_servers = 35
```

## Related Use Cases

- [UC02: HTTP LB](./uc02-http-lb.md) - Pure HTTP load balancing
- [UC11: CDN Edge](./uc11-cdn-edge.md) - Add caching
