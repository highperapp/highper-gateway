# Test PHP-FPM and Static Files Configuration

log info
metrics prometheus port=9090

# PHP-FPM Web Server
http://php.example.com:8080 {
    root "/var/www/html"
    index index.php index.html

    # Static files
    /static/* {
        static_files
        try_files $uri =404
    }

    # PHP scripts
    /*.php {
        php_fpm enabled
        php_fpm socket="/var/run/php/php8.2-fpm.sock"
        php_fpm pool_size=50
        php_fpm connect_timeout=5s
        php_fpm read_timeout=60s
        php_fpm write_timeout=60s
        php_fpm keepalive=90s
        php_fpm script_extensions .php .phtml
        proxy localhost:9000
    }

    # Default route - try files or pass to PHP
    /* {
        try_files $uri $uri/ /index.php
    }
}

# Simple static file server
http://static.example.com:8081 {
    root "/var/www/static"
    index index.html index.htm
    static_files
}
