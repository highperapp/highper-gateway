# Highper Gateway DSL Configuration
# Use Case 10: Hybrid Multi-Protocol Gateway

server {
  name = "highper-gateway-hybrid"
}

io_uring {
  entries     = 8192
  sq_poll     = true
  sq_poll_cpu = 0
}

tls "default" {
  cert = "/etc/highper-gateway/certs/server.crt"
  key  = "/etc/highper-gateway/certs/server.key"
}

# HTTP traffic
listener "http-main" {
  protocol = "http"
  bind     = "0.0.0.0:80"

  route {
    match {
      path_prefix = "/api"
    }
    backend = "api-servers"
  }

  route {
    match {
      path_prefix = "/ws"
      header {
        name  = "Upgrade"
        value = "websocket"
      }
    }
    backend = "ws-servers"
  }

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

# HTTPS traffic
listener "https-main" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  tls      = "default"

  http2 {
    enabled = true
  }

  websocket {
    enabled = true
  }

  route {
    match {
      path_prefix = "/api/v1"
    }
    backend = "api-v1-servers"
    rate_limit {
      requests_per_second = 100
    }
  }

  route {
    match {
      path_prefix = "/api/v2"
    }
    backend = "api-v2-servers"
    rate_limit {
      requests_per_second = 200
    }
  }

  route {
    match {
      path_prefix = "/ws"
    }
    backend = "ws-servers"
  }

  route {
    match {
      path_prefix = "/graphql"
    }
    backend = "graphql-servers"
  }

  route {
    match {
      path_prefix = "/"
    }
    backend = "web-servers"
  }
}

# Internal HTTP API
listener "http-internal" {
  protocol = "http"
  bind     = "0.0.0.0:8080"

  route {
    match {
      path_prefix = "/internal"
    }
    backend = "internal-servers"
  }

  route {
    match {
      path_prefix = "/"
    }
    backend = "api-servers"
  }
}

# gRPC traffic
listener "grpc-main" {
  protocol = "grpc"
  bind     = "0.0.0.0:9090"

  route {
    match {
      service = "*"
    }
    backend = "grpc-servers"
  }
}

backend "web-servers" {
  strategy = "round_robin"

  server {
    address = "web-1.app.local:8080"
  }

  server {
    address = "web-2.app.local:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }
}

backend "api-v1-servers" {
  strategy = "least_conn"

  server {
    address = "api-v1-1.app.local:8080"
  }

  server {
    address = "api-v1-2.app.local:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 5
  }
}

backend "api-v2-servers" {
  strategy = "least_conn"

  server {
    address = "api-v2-1.app.local:8080"
  }

  server {
    address = "api-v2-2.app.local:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 5
  }
}

backend "api-servers" {
  strategy = "round_robin"

  server {
    address = "api-1.app.local:8080"
  }

  server {
    address = "api-2.app.local:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }
}

backend "ws-servers" {
  strategy = "ip_hash"

  server {
    address = "ws-1.app.local:8080"
  }

  server {
    address = "ws-2.app.local:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }
}

backend "graphql-servers" {
  strategy = "round_robin"

  server {
    address = "graphql-1.app.local:4000"
  }

  server {
    address = "graphql-2.app.local:4000"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }
}

backend "grpc-servers" {
  strategy = "round_robin"
  protocol = "grpc"

  server {
    address = "grpc-1.app.local:9090"
  }

  server {
    address = "grpc-2.app.local:9090"
  }

  health_check {
    enabled  = true
    protocol = "grpc"
    service  = "grpc.health.v1.Health"
    interval = 10
  }
}

backend "internal-servers" {
  strategy = "round_robin"

  server {
    address = "internal-1.app.local:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 30
  }
}

logging {
  level  = "info"
  format = "json"
  output = "/var/log/highper-gateway/gateway.log"
}

metrics {
  enabled  = true
  endpoint = "/metrics"
  port     = 9091
}

health {
  enabled  = true
  endpoint = "/health"
  port     = 8081
}
