log info

# Simple PHP-FPM test configuration
http://localhost:8080 {
    root "/var/www/html"
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
