# Highper Gateway DSL Configuration
# Use Case 11: CDN Edge Caching

server {
  name = "highper-gateway-cdn-edge"
}

io_uring {
  entries     = 8192
  sq_poll     = true
  sq_poll_cpu = 0
}

tls "default" {
  cert            = "/etc/highper-gateway/certs/server.crt"
  key             = "/etc/highper-gateway/certs/server.key"
  session_cache   = "shared"
  session_tickets = true
}

listener "http-edge" {
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

listener "https-edge" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  tls      = "default"

  http2 {
    enabled = true
  }

  route {
    match {
      path_prefix = "/static"
    }
    backend = "origin-static"
    cache {
      enabled                = true
      ttl                    = 86400
      stale_while_revalidate = 3600
    }
  }

  route {
    match {
      path_prefix = "/assets"
    }
    backend = "origin-assets"
    cache {
      enabled = true
      ttl     = 604800
    }
  }

  route {
    match {
      path_prefix = "/api"
    }
    backend = "origin-api"
    cache {
      enabled = false
    }
  }

  route {
    match {
      path_prefix = "/"
    }
    backend = "origin-dynamic"
    cache {
      enabled = true
      ttl     = 300
      vary    = ["Accept-Encoding", "Accept-Language"]
    }
  }
}

backend "origin-static" {
  strategy = "round_robin"

  server {
    address = "origin-1.cdn.local:8080"
  }

  server {
    address = "origin-2.cdn.local:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 30
  }
}

backend "origin-assets" {
  strategy = "round_robin"

  server {
    address = "assets-origin.cdn.local:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 60
  }
}

backend "origin-api" {
  strategy = "least_conn"

  server {
    address = "api-origin-1.cdn.local:8080"
  }

  server {
    address = "api-origin-2.cdn.local:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }
}

backend "origin-dynamic" {
  strategy = "round_robin"

  server {
    address = "dynamic-origin.cdn.local:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 15
  }
}

cache {
  enabled         = true
  default_ttl     = 3600
  max_object_size = 10485760  # 10MB
  stale_if_error  = 86400
  stale_while_revalidate = 60

  storage {
    type = "memory"
    size = 1073741824  # 1GB
  }

  bypass_headers     = ["Authorization", "Cookie"]
  cacheable_methods  = ["GET", "HEAD"]
  respect_cache_control = true
  respect_vary          = true
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
    "image/svg+xml"
  ]
}

headers {
  response {
    add {
      name  = "X-Cache-Status"
      value = "$cache_status"
    }

    add {
      name  = "X-Edge-Location"
      value = "$edge_location"
    }

    remove = ["X-Powered-By", "Server"]
  }
}

logging {
  level  = "info"
  format = "json"
  output = "/var/log/highper-gateway/gateway.log"

  access_log {
    enabled = true
    output  = "/var/log/highper-gateway/access.log"
    fields  = ["timestamp", "method", "path", "status", "response_time", "cache_status", "bytes_sent"]
  }
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
