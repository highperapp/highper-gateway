# PHP-FPM and Static File Serving Scenarios
# Demonstrates various PHP-FPM configurations for the highper-gateway

log info
metrics prometheus port=9090

# ==============================================================================
# Scenario 1: Simple PHP-FPM Application
# ==============================================================================
# A basic PHP application with static file support
http://simple-php.example.com:8080 {
    root "/var/www/simple"
    index index.php index.html

    # Serve static files
    /static/* {
        static_files
        try_files $uri =404
    }

    # PHP scripts
    /*.php {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=50
        proxy localhost:9000
    }

    # Default route - try files or fall back to index.php
    /* {
        try_files $uri $uri/ /index.php
    }
}

# ==============================================================================
# Scenario 2: WordPress Site
# ==============================================================================
# WordPress with pretty permalinks and static asset optimization
https://blog.example.com {
    root "/var/www/wordpress"
    index index.php

    # Static assets (CSS, JS, images)
    /wp-content/* {
        static_files
        try_files $uri =404
    }

    /wp-includes/* {
        static_files
        try_files $uri =404
    }

    # WordPress admin
    /wp-admin/*.php {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=100 read_timeout=120s
        proxy localhost:9000
    }

    # All PHP files
    /*.php {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=100
        proxy localhost:9000
    }

    # Pretty permalinks - try file, directory, then index.php
    /* {
        try_files $uri $uri/ /index.php
    }

    tls admin@example.com
    cors origins="https://blog.example.com" credentials
}

# ==============================================================================
# Scenario 3: Laravel Application
# ==============================================================================
# Laravel framework with public directory and API rate limiting
https://api.laravel.example.com {
    root "/var/www/laravel/public"
    index index.php

    # Static assets in public directory
    /css/* {
        static_files
    }

    /js/* {
        static_files
    }

    /images/* {
        static_files
    }

    # API routes with rate limiting
    /api/* {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=200 read_timeout=60s keepalive=120s
        rate_limit 1000 burst=100 per 1m per_ip
        proxy localhost:9000
    }

    # All other PHP requests
    /*.php {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=200
        proxy localhost:9000
    }

    # Laravel routing - all requests go to index.php
    /* {
        try_files $uri $uri/ /index.php
    }

    tls internal
    compress gzip br
    timeout 90s
}

# ==============================================================================
# Scenario 4: Multi-Version PHP
# ==============================================================================
# Serve different PHP versions for different applications
http://php-multi.example.com:8081 {
    root "/var/www/multi"

    # PHP 8.2 application
    /app82/* {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=50
        proxy localhost:9000
    }

    # PHP 8.1 application
    /app81/* {
        php_fpm enabled socket="/var/run/php/php8.1-fpm.sock" pool_size=50
        proxy localhost:9001
    }

    # PHP 7.4 legacy application
    /legacy/* {
        php_fpm enabled socket="/var/run/php/php7.4-fpm.sock" pool_size=30
        proxy localhost:9002
    }
}

# ==============================================================================
# Scenario 5: High-Performance PHP Application
# ==============================================================================
# Optimized configuration for high-traffic PHP application
https://high-performance.example.com {
    root "/var/www/high-perf"
    index index.php

    # Static assets with aggressive caching
    /assets/* {
        static_files
        cache enabled ttl=1h max_size=10000
        compress gzip br zstd
    }

    # PHP with optimized pool settings
    /*.php {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=500 connect_timeout=3s read_timeout=30s write_timeout=30s keepalive=120s script_extensions .php .phtml
        pool max=500 min=100 lifetime=1h
        health interval=10s timeout=5s path=/health.php
        proxy backend1:9000 backend2:9000 backend3:9000
        lb least_conn
    }

    # Default routing
    /* {
        try_files $uri $uri/ /index.php
    }

    tls "certs/server.crt" "certs/server.key"
    compress gzip br level=6
    keepalive 120s
    max_conns 10000
}

# ==============================================================================
# Scenario 6: Development Environment
# ==============================================================================
# Development setup with hot-reload support
http://dev.local:8080 {
    root "/var/www/dev"
    index index.php index.html

    # All PHP files with development settings
    /*.php {
        php_fpm enabled socket="/var/run/php/php-fpm.sock" pool_size=10 read_timeout=300s
        proxy localhost:9000
    }

    # Static files
    /* {
        static_files
        try_files $uri $uri/ /index.php
    }
}

# ==============================================================================
# Scenario 7: Static Site with PHP Contact Form
# ==============================================================================
# Mostly static content with a single PHP endpoint
http://static-with-contact.example.com:8082 {
    root "/var/www/static-site"
    index index.html

    # Contact form PHP endpoint
    /contact.php {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=10
        rate_limit 10 per 1m per_ip
        proxy localhost:9000
    }

    # Everything else is static
    /* {
        static_files
        try_files $uri $uri/ /index.html
    }

    cors
}

# ==============================================================================
# Scenario 8: API Gateway with PHP Backend
# ==============================================================================
# RESTful API with advanced features
https://api.gateway.example.com {
    # PHP API endpoints
    /v1/* {
        php_fpm enabled socket="/var/run/php/php8.2-fpm.sock" pool_size=300 read_timeout=60s
        rate_limit 5000 burst=500 per 1h per_ip
        cors origins="https://app.example.com" methods="GET,POST,PUT,DELETE" headers="Content-Type,Authorization" credentials
        proxy api-backend1:9000 api-backend2:9000
        lb round_robin
        health interval=15s timeout=5s path=/v1/health
        timeout 60s
        circuit_breaker threshold=10 timeout=30s window=60s
    }

    # API documentation (static)
    /docs/* {
        static_files
        root "/var/www/api-docs"
        try_files $uri $uri/ /index.html
    }

    tls admin@api.example.com
    compress gzip br
}
