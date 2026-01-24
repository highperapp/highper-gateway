# Highper Gateway DSL Configuration
# Use Case 04: API Gateway with Rate Limiting

server {
  name = "highper-gateway-api-gw"
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

listener "api-https" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  tls      = "default"

  route {
    match {
      path_prefix = "/v1/users"
    }
    backend = "user-service"
    rate_limit {
      requests_per_second = 100
      burst               = 20
    }
  }

  route {
    match {
      path_prefix = "/v1/products"
    }
    backend = "product-service"
    rate_limit {
      requests_per_second = 500
      burst               = 100
    }
  }

  route {
    match {
      path_prefix = "/v1/orders"
    }
    backend = "order-service"
    rate_limit {
      requests_per_second = 200
      burst               = 50
    }
    auth {
      required = true
    }
  }
}

listener "api-http" {
  protocol = "http"
  bind     = "0.0.0.0:8080"

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

backend "user-service" {
  strategy = "round_robin"

  server {
    address = "user-service.default.svc:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }

  timeout = 5000

  retry {
    attempts   = 3
    conditions = ["connection_error", "502", "503"]
  }
}

backend "product-service" {
  strategy = "least_conn"

  server {
    address = "product-service.default.svc:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }

  timeout = 10000
}

backend "order-service" {
  strategy = "round_robin"

  server {
    address = "order-service.default.svc:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 5
  }

  timeout = 15000
}

rate_limiting {
  enabled      = true
  by_client_ip = true

  default {
    requests_per_second = 1000
    burst               = 200
  }
}

auth {
  jwt {
    enabled    = true
    secret_env = "JWT_SECRET"
    algorithms = ["HS256", "RS256"]
    header     = "Authorization"
    prefix     = "Bearer"
  }
}

cors {
  enabled = true
  allow_origins = [
    "https://example.com",
    "https://*.example.com"
  ]
  allow_methods = ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
  allow_headers = ["Authorization", "Content-Type", "X-Request-ID"]
  max_age       = 3600
}

transform {
  request {
    headers {
      add "X-Gateway-Version" = "1.0"
      add "X-Request-ID"      = "$request_id"
      remove                  = ["X-Internal-Header"]
    }
  }

  response {
    headers {
      add "X-Response-Time" = "$response_time"
      remove                = ["Server"]
    }
  }
}

logging {
  level  = "info"
  format = "json"
  output = "/var/log/highper-gateway/gateway.log"

  access_log {
    enabled = true
    output  = "/var/log/highper-gateway/access.log"
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
