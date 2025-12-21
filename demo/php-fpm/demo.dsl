# PHP-FPM Demo Configuration
#
# This configuration demonstrates PHP-FPM and static file serving
# with Highper Gateway.
#
# Usage: highper-gateway --config demo/php-fpm/demo.dsl

log info
metrics prometheus port=9090

# Main PHP application
http://localhost:8080 {
    root "demo/php-fpm"
    index index.php index.html

    # Custom error pages
    error_page 404 "/404.html"
    error_page 500 "/500.html"

    # Static files in /static directory
    /static/* {
        static_files
        try_files $uri =404
    }

    # PHP files via FastCGI
    /*.php {
        php_fpm enabled socket="/var/run/php/php-fpm.sock" pool_size=50 read_timeout=60s
        proxy localhost:9000
    }

    # Default routing with index.php fallback
    /* {
        try_files $uri $uri/ /index.php
    }
}

# Alternative: TCP socket instead of Unix socket
# (Uncomment if using PHP-FPM on TCP port 9000)
#
# http://localhost:8081 {
#     root "demo/php-fpm"
#     index index.php
#
#     /*.php {
#         php_fpm enabled socket="127.0.0.1:9000" pool_size=50
#         proxy localhost:9000
#     }
# }
