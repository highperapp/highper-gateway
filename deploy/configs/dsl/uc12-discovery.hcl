# Highper Gateway DSL Configuration
# Use Case 12: Microservices Service Discovery

server {
  name = "highper-gateway-discovery"
}

io_uring {
  entries     = 4096
  sq_poll     = true
  sq_poll_cpu = 0
}

listener "http-gateway" {
  protocol = "http"
  bind     = "0.0.0.0:8080"

  route {
    match {
      path_prefix = "/users"
    }
    backend = "user-service"
  }

  route {
    match {
      path_prefix = "/products"
    }
    backend = "product-service"
  }

  route {
    match {
      path_prefix = "/orders"
    }
    backend = "order-service"
  }

  route {
    match {
      path_prefix = "/payments"
    }
    backend = "payment-service"
  }

  route {
    match {
      path_prefix = "/"
    }
    backend = "frontend-service"
  }
}

listener "consul-proxy" {
  protocol = "http"
  bind     = "0.0.0.0:8500"

  route {
    match {
      path_prefix = "/"
    }
    backend = "consul-servers"
  }
}

discovery {
  provider = "consul"

  consul {
    address             = "consul.service.local:8500"
    datacenter          = "dc1"
    token_env           = "CONSUL_TOKEN"
    refresh_interval    = 10
    health_check_filter = "passing"
  }
}

backend "user-service" {
  strategy = "round_robin"

  discovery {
    service = "user-service"
    tags    = ["production"]
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }
}

backend "product-service" {
  strategy = "least_conn"

  discovery {
    service = "product-service"
    tags    = ["production"]
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }
}

backend "order-service" {
  strategy = "round_robin"

  discovery {
    service = "order-service"
    tags    = ["production"]
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 5
  }
}

backend "payment-service" {
  strategy = "round_robin"

  discovery {
    service = "payment-service"
    tags    = ["production"]
  }

  circuit_breaker {
    enabled   = true
    threshold = 5
    timeout   = 30000
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 5
  }
}

backend "frontend-service" {
  strategy = "round_robin"

  discovery {
    service = "frontend-service"
    tags    = ["production"]
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 15
  }
}

backend "consul-servers" {
  strategy = "round_robin"

  server {
    address = "consul-1.service.local:8500"
  }

  server {
    address = "consul-2.service.local:8500"
  }

  server {
    address = "consul-3.service.local:8500"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/v1/status/leader"
    interval = 10
  }
}

service_mesh {
  tracing {
    enabled     = true
    provider    = "jaeger"
    endpoint    = "http://jaeger.monitoring.local:14268/api/traces"
    sample_rate = 0.1
  }

  retry {
    enabled          = true
    attempts         = 3
    per_try_timeout  = 5000
    retry_on         = ["connection_error", "502", "503", "504"]
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
  port     = 9090
}

health {
  enabled  = true
  endpoint = "/health"
  port     = 8081
}
