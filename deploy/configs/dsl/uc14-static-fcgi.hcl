# Highper Gateway DSL Configuration
# Use Case 14: Static Files + FastCGI (PHP-FPM)

server {
  name = "highper-gateway-static-fcgi"
}

io_uring {
  entries     = 4096
  sq_poll     = true
  sq_poll_cpu = 0
}

tls "default" {
  cert = "/etc/highper-gateway/certs/server.crt"
  key  = "/etc/highper-gateway/certs/server.key"
}

listener "http-main" {
  protocol = "http"
  bind     = "0.0.0.0:80"

  route {
    match {
      path_prefix = "/"
    }
    redirect {
      to   = "https"
      code = 301
    }
  }
}

listener "https-main" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  tls      = "default"

  # Static assets
  route {
    match {
      path_regex = "\\.(css|js|jpg|jpeg|png|gif|ico|svg|woff|woff2|ttf|eot)$"
    }
    static {
      root          = "/var/www/html"
      cache_control = "public, max-age=31536000"
    }
    cache {
      enabled = true
      ttl     = 86400
    }
  }

  # PHP files via FastCGI
  route {
    match {
      path_regex = "\\.php$"
    }
    fastcgi {
      backend         = "php-fpm"
      script_filename = "/var/www/html$uri"

      params {
        SCRIPT_FILENAME = "/var/www/html$uri"
        SCRIPT_NAME     = "$uri"
        REQUEST_URI     = "$uri"
        QUERY_STRING    = "$query_string"
        REQUEST_METHOD  = "$method"
        CONTENT_TYPE    = "$content_type"
        CONTENT_LENGTH  = "$content_length"
      }
    }
  }

  # Default: try static file, then index.php
  route {
    match {
      path_prefix = "/"
    }
    static {
      root  = "/var/www/html"
      index = ["index.html", "index.php"]
      try_files = ["$uri", "$uri/", "/index.php$is_args$args"]
    }
    fallback {
      fastcgi {
        backend         = "php-fpm"
        script_filename = "/var/www/html/index.php"
      }
    }
  }
}

backend "php-fpm" {
  protocol = "fastcgi"
  strategy = "round_robin"

  server {
    address = "unix:/var/run/php/php-fpm.sock"
  }

  health_check {
    enabled  = true
    protocol = "fastcgi"
    script   = "/var/www/html/ping.php"
    interval = 10
  }

  connection {
    timeout         = 30000
    max_connections = 100
  }
}

static {
  root              = "/var/www/html"
  index             = ["index.html", "index.htm", "index.php"]
  directory_listing = false
  etag              = true
  last_modified     = true
}

compression {
  enabled    = true
  algorithms = ["br", "gzip"]
  min_size   = 1024

  types = [
    "text/html",
    "text/css",
    "text/javascript",
    "application/javascript",
    "application/json",
    "text/xml",
    "application/xml",
    "image/svg+xml"
  ]
}

headers {
  response {
    add {
      name  = "X-Content-Type-Options"
      value = "nosniff"
    }

    add {
      name  = "X-Frame-Options"
      value = "SAMEORIGIN"
    }

    add {
      name  = "X-XSS-Protection"
      value = "1; mode=block"
    }

    add {
      name  = "Referrer-Policy"
      value = "strict-origin-when-cross-origin"
    }

    remove = ["X-Powered-By", "Server"]
  }
}

php {
  max_execution_time  = 30
  max_input_vars      = 1000
  upload_max_filesize = 10485760
  post_max_size       = 10485760
}

logging {
  level  = "info"
  format = "json"
  output = "/var/log/highper-gateway/gateway.log"
}

metrics {
  enabled  = true
  endpoint = "/metrics"
  port     = 9090
}

health {
  enabled  = true
  endpoint = "/health"
  port     = 8081
}
