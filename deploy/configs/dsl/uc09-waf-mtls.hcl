# Highper Gateway DSL Configuration
# Use Case 09: WAF + mTLS

server {
  name = "highper-gateway-waf-mtls"
}

io_uring {
  entries     = 4096
  sq_poll     = true
  sq_poll_cpu = 0
}

tls "mtls" {
  cert         = "/etc/highper-gateway/certs/server.crt"
  key          = "/etc/highper-gateway/certs/server.key"
  ca           = "/etc/highper-gateway/certs/ca.crt"
  client_auth  = "required"
  verify_depth = 3
  protocols    = ["TLSv1.2", "TLSv1.3"]

  crl {
    enabled        = true
    path           = "/etc/highper-gateway/certs/crl.pem"
    check_interval = 3600
  }
}

listener "secure-gateway" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  tls      = "mtls"

  waf {
    enabled = true
    ruleset = "/etc/highper-gateway/rules/waf.rules"
  }

  route {
    match {
      path_prefix = "/api"
    }
    backend = "api-servers"
    waf {
      mode = "block"
    }
  }

  route {
    match {
      path_prefix = "/"
    }
    backend = "web-servers"
    waf {
      mode = "detect"
    }
  }
}

backend "api-servers" {
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

waf {
  enabled     = true
  mode        = "block"
  ruleset_dir = "/etc/highper-gateway/rules"

  rules {
    sql_injection {
      enabled     = true
      sensitivity = "high"
    }

    xss {
      enabled     = true
      sensitivity = "high"
    }

    path_traversal {
      enabled = true
    }

    command_injection {
      enabled = true
    }

    request_limits {
      max_request_size = 10485760
      max_uri_length   = 8192
      max_header_size  = 16384
      max_headers      = 100
    }
  }

  ip_reputation {
    enabled   = true
    blocklist = "/etc/highper-gateway/rules/ip-blocklist.txt"
    allowlist = "/etc/highper-gateway/rules/ip-allowlist.txt"
  }

  logging {
    enabled     = true
    log_blocked = true
    log_detected = true
    output      = "/var/log/highper-gateway/waf.log"
  }
}

client_cert {
  extract_dn = true

  headers {
    add "X-Client-Cert-DN"          = "$client_cert_dn"
    add "X-Client-Cert-Serial"      = "$client_cert_serial"
    add "X-Client-Cert-Fingerprint" = "$client_cert_fingerprint"
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
