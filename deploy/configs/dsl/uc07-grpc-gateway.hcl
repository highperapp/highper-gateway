# Highper Gateway DSL Configuration
# Use Case 07: gRPC Gateway

server {
  name = "highper-gateway-grpc"
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

listener "grpc-main" {
  protocol = "grpc"
  bind     = "0.0.0.0:9090"

  route {
    match {
      service = "user.UserService"
    }
    backend = "user-grpc"
  }

  route {
    match {
      service = "product.ProductService"
    }
    backend = "product-grpc"
  }

  route {
    match {
      service = "order.OrderService"
    }
    backend = "order-grpc"
  }

  route {
    match {
      service = "*"
    }
    backend = "default-grpc"
  }
}

listener "grpc-tls" {
  protocol = "grpc"
  bind     = "0.0.0.0:443"
  tls      = "default"

  route {
    match {
      service = "*"
    }
    backend = "default-grpc"
  }
}

backend "user-grpc" {
  strategy = "round_robin"
  protocol = "grpc"

  server {
    address = "user-service:9090"
  }

  server {
    address = "user-service-2:9090"
  }

  health_check {
    enabled  = true
    protocol = "grpc"
    service  = "grpc.health.v1.Health"
    interval = 10
    timeout  = 5
  }
}

backend "product-grpc" {
  strategy = "least_conn"
  protocol = "grpc"

  server {
    address = "product-service:9090"
  }

  health_check {
    enabled  = true
    protocol = "grpc"
    service  = "grpc.health.v1.Health"
    interval = 10
  }
}

backend "order-grpc" {
  strategy = "round_robin"
  protocol = "grpc"

  server {
    address = "order-service:9090"
  }

  health_check {
    enabled  = true
    protocol = "grpc"
    service  = "grpc.health.v1.Health"
    interval = 5
  }
}

backend "default-grpc" {
  strategy = "round_robin"
  protocol = "grpc"

  server {
    address = "default-service:9090"
  }
}

grpc {
  max_message_size = 16777216

  keepalive {
    time                  = 30000
    timeout               = 5000
    permit_without_stream = true
  }

  compression {
    enabled    = true
    algorithms = ["gzip", "snappy"]
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
