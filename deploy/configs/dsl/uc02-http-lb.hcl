# Highper Gateway DSL Configuration
# Use Case 02: Layer 7 HTTP Load Balancer

server {
  name = "highper-gateway-http-lb"
}

io_uring {
  entries     = 4096
  sq_poll     = true
  sq_poll_cpu = 0
}

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
      path_prefix = "/"
    }
    backend = "web-servers"
  }
}

listener "http-admin" {
  protocol = "http"
  bind     = "0.0.0.0:8080"

  route {
    match {
      path_prefix = "/"
    }
    backend = "admin-servers"
  }
}

backend "web-servers" {
  strategy = "round_robin"

  server {
    address = "10.0.1.10:8080"
    weight  = 1
  }

  server {
    address = "10.0.1.11:8080"
    weight  = 1
  }

  server {
    address = "10.0.1.12:8080"
    weight  = 1
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
    timeout  = 5
  }
}

backend "api-servers" {
  strategy = "least_conn"

  server {
    address = "10.0.2.10:8080"
  }

  server {
    address = "10.0.2.11:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/api/health"
    interval = 5
    timeout  = 3
  }
}

backend "admin-servers" {
  strategy = "round_robin"

  server {
    address = "10.0.3.10:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 30
  }
}

http {
  keepalive         = true
  keepalive_timeout = 60
  request_timeout   = 30000
  max_request_size  = 10485760
}

headers {
  request {
    add "X-Forwarded-Proto" = "http"
    add "X-Request-ID"      = "$request_id"
  }

  response {
    add "X-Served-By" = "highper-gateway"
  }
}

logging {
  level  = "info"
  format = "json"
  output = "/var/log/highper-gateway/gateway.log"

  access_log {
    enabled = true
    output  = "/var/log/highper-gateway/access.log"
    format  = "combined"
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
