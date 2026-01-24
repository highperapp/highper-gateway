# Highper Gateway DSL Configuration
# Use Case 06: WebSocket Load Balancer

server {
  name = "highper-gateway-websocket"
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

listener "ws-http" {
  protocol = "http"
  bind     = "0.0.0.0:80"

  websocket {
    enabled       = true
    ping_interval = 30
    ping_timeout  = 10
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
    backend = "web-servers"
  }
}

listener "wss-https" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  tls      = "default"

  websocket {
    enabled       = true
    ping_interval = 30
    ping_timeout  = 10
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
    backend = "web-servers"
  }
}

listener "ws-dedicated" {
  protocol = "http"
  bind     = "0.0.0.0:8080"

  websocket {
    enabled          = true
    ping_interval    = 30
    ping_timeout     = 10
    max_frame_size   = 65536
    max_message_size = 1048576
  }

  route {
    match {
      path_prefix = "/"
    }
    backend = "ws-servers"
  }
}

backend "ws-servers" {
  strategy = "ip_hash"  # Session affinity for WebSocket

  server {
    address = "10.0.1.10:8080"
  }

  server {
    address = "10.0.1.11:8080"
  }

  server {
    address = "10.0.1.12:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }

  connection {
    keepalive = true
    timeout   = 3600000  # 1 hour for long-lived connections
  }
}

backend "web-servers" {
  strategy = "round_robin"

  server {
    address = "10.0.2.10:8080"
  }

  server {
    address = "10.0.2.11:8080"
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }
}

websocket {
  handshake_timeout = 10000

  compression {
    enabled = true
    level   = 6
  }

  subprotocols = ["graphql-ws", "graphql-transport-ws"]
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
