# Highper Gateway DSL Configuration
# Use Case 03: HTTPS/TLS Termination

server {
  name = "highper-gateway-https"
}

io_uring {
  entries     = 4096
  sq_poll     = true
  sq_poll_cpu = 0
}

tls "default" {
  cert = "/etc/highper-gateway/certs/server.crt"
  key  = "/etc/highper-gateway/certs/server.key"

  protocols = ["TLSv1.2", "TLSv1.3"]

  ciphers = [
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256",
    "TLS_AES_128_GCM_SHA256",
    "ECDHE-ECDSA-AES256-GCM-SHA384",
    "ECDHE-RSA-AES256-GCM-SHA384"
  ]

  prefer_server_ciphers = true
  session_cache         = "shared"
  session_timeout       = 3600
  ocsp_stapling         = true
}

listener "https-main" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  tls      = "default"

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

listener "https-alt" {
  protocol = "https"
  bind     = "0.0.0.0:8443"
  tls      = "default"

  route {
    match {
      path_prefix = "/"
    }
    backend = "internal-servers"
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

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
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
  }
}

backend "internal-servers" {
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

hsts {
  enabled            = true
  max_age            = 31536000
  include_subdomains = true
  preload            = true
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
