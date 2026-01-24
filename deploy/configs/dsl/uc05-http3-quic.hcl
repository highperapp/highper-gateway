# Highper Gateway DSL Configuration
# Use Case 05: HTTP/3 QUIC

server {
  name = "highper-gateway-http3"
}

io_uring {
  entries     = 8192
  sq_poll     = true
  sq_poll_cpu = 0
}

tls "default" {
  cert = "/etc/highper-gateway/certs/server.crt"
  key  = "/etc/highper-gateway/certs/server.key"

  protocols = ["TLSv1.3"]  # HTTP/3 requires TLS 1.3

  alpn = ["h3", "h2", "http/1.1"]
}

listener "http3-main" {
  protocol = "http3"
  bind     = "0.0.0.0:443"
  tls      = "default"

  quic {
    max_idle_timeout                  = 30000
    max_udp_payload_size              = 1350
    initial_max_data                  = 10485760
    initial_max_stream_data_bidi_local  = 1048576
    initial_max_stream_data_bidi_remote = 1048576
    initial_max_streams_bidi          = 100
    initial_max_streams_uni           = 100
    ack_delay_exponent                = 3
    max_ack_delay                     = 25
  }

  route {
    match {
      path_prefix = "/"
    }
    backend = "web-servers"
  }
}

listener "https-fallback" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  tls      = "default"

  http2 {
    enabled               = true
    max_concurrent_streams = 100
  }

  route {
    match {
      path_prefix = "/"
    }
    backend = "web-servers"
  }
}

backend "web-servers" {
  strategy = "round_robin"

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
}

headers {
  response {
    add {
      name  = "Alt-Svc"
      value = "h3=\":443\"; ma=86400"
    }
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
