# Highper Gateway DSL Configuration
# Use Case 13: GraphQL Gateway

server {
  name = "highper-gateway-graphql"
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

listener "graphql-http" {
  protocol = "http"
  bind     = "0.0.0.0:4000"

  route {
    match {
      path    = "/graphql"
      methods = ["POST", "GET"]
    }
    backend = "graphql-servers"
    graphql {
      enabled = true
    }
  }

  route {
    match {
      path = "/graphiql"
    }
    backend = "graphql-servers"
  }

  route {
    match {
      path = "/subscriptions"
      header {
        name  = "Upgrade"
        value = "websocket"
      }
    }
    backend = "graphql-servers"
    websocket {
      enabled = true
    }
  }
}

listener "graphql-https" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  tls      = "default"

  websocket {
    enabled = true
  }

  route {
    match {
      path = "/graphql"
    }
    backend = "graphql-servers"
    graphql {
      enabled = true
    }
  }

  route {
    match {
      path = "/subscriptions"
    }
    backend = "graphql-servers"
    websocket {
      enabled = true
    }
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

  connection {
    timeout   = 30000
    keepalive = true
  }
}

graphql {
  enabled = true

  introspection {
    enabled   = true
    cache_ttl = 300
  }

  complexity {
    enabled        = true
    max_depth      = 10
    max_complexity = 1000
    max_aliases    = 10
  }

  persisted_queries {
    enabled    = true
    cache_size = 10000
  }

  rate_limiting {
    enabled = true

    default {
      requests_per_second = 100
      burst               = 20
    }

    by_operation {
      operation           = "heavyQuery"
      requests_per_second = 10
      burst               = 5
    }
  }

  batching {
    enabled        = true
    max_batch_size = 10
  }

  cache {
    enabled        = true
    ttl            = 60
    private_fields = ["user", "me", "viewer"]
  }
}

cors {
  enabled = true

  allow_origins = ["*"]
  allow_methods = ["GET", "POST", "OPTIONS"]
  allow_headers = ["Content-Type", "Authorization", "X-Apollo-Operation-Name"]
  max_age       = 3600
}

logging {
  level  = "info"
  format = "json"
  output = "/var/log/highper-gateway/gateway.log"

  graphql_logging {
    enabled       = true
    log_queries   = true
    log_variables = false  # Privacy
  }
}

metrics {
  enabled  = true
  endpoint = "/metrics"
  port     = 9090

  graphql_metrics {
    enabled           = true
    operation_latency = true
    field_usage       = true
  }
}

health {
  enabled  = true
  endpoint = "/health"
  port     = 8081
}
