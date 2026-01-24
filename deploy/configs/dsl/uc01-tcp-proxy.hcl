# Highper Gateway DSL Configuration
# Use Case 01: Layer 4 TCP Proxy

server {
  name = "highper-gateway-tcp-proxy"
}

io_uring {
  entries     = 8192
  sq_poll     = true
  sq_poll_cpu = 0
}

listener "http-proxy" {
  protocol = "tcp"
  bind     = "0.0.0.0:80"
  backend  = "web-servers"
}

listener "https-proxy" {
  protocol = "tcp"
  bind     = "0.0.0.0:443"
  backend  = "web-servers-tls"
}

listener "mysql-proxy" {
  protocol = "tcp"
  bind     = "0.0.0.0:3306"
  backend  = "mysql-servers"
}

listener "postgres-proxy" {
  protocol = "tcp"
  bind     = "0.0.0.0:5432"
  backend  = "postgres-servers"
}

backend "web-servers" {
  strategy = "round_robin"

  server {
    address = "10.0.1.10:80"
    weight  = 1
  }

  server {
    address = "10.0.1.11:80"
    weight  = 1
  }

  health_check {
    enabled  = true
    protocol = "tcp"
    interval = 10
    timeout  = 5
  }
}

backend "web-servers-tls" {
  strategy = "round_robin"

  server {
    address = "10.0.1.10:443"
  }

  server {
    address = "10.0.1.11:443"
  }

  health_check {
    enabled  = true
    protocol = "tcp"
    interval = 10
  }
}

backend "mysql-servers" {
  strategy = "least_conn"

  server {
    address = "10.0.2.10:3306"
    weight  = 2
  }

  server {
    address = "10.0.2.11:3306"
    weight  = 1
  }

  health_check {
    enabled  = true
    protocol = "tcp"
    interval = 5
    timeout  = 2
  }
}

backend "postgres-servers" {
  strategy = "least_conn"

  server {
    address = "10.0.3.10:5432"
  }

  server {
    address = "10.0.3.11:5432"
  }

  health_check {
    enabled  = true
    protocol = "tcp"
    interval = 5
  }
}

connection {
  keepalive       = true
  timeout         = 30000
  max_connections = 65535
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
