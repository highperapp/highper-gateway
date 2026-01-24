# Highper Gateway DSL Configuration
# Use Case 08: Database Load Balancer

server {
  name = "highper-gateway-database"
}

io_uring {
  entries     = 4096
  sq_poll     = true
  sq_poll_cpu = 0
}

# MySQL/MariaDB
listener "mysql-primary" {
  protocol = "tcp"
  bind     = "0.0.0.0:3306"
  backend  = "mysql-pool"

  connection {
    timeout   = 30000
    keepalive = true
  }
}

# PostgreSQL
listener "postgres-primary" {
  protocol = "tcp"
  bind     = "0.0.0.0:5432"
  backend  = "postgres-pool"

  connection {
    timeout   = 30000
    keepalive = true
  }
}

# Redis
listener "redis-primary" {
  protocol = "tcp"
  bind     = "0.0.0.0:6379"
  backend  = "redis-pool"

  connection {
    timeout   = 10000
    keepalive = true
  }
}

backend "mysql-pool" {
  strategy = "least_conn"

  server {
    address = "mysql-primary.db.local:3306"
    weight  = 10
    role    = "primary"
  }

  server {
    address = "mysql-replica-1.db.local:3306"
    weight  = 5
    role    = "replica"
  }

  server {
    address = "mysql-replica-2.db.local:3306"
    weight  = 5
    role    = "replica"
  }

  health_check {
    enabled   = true
    protocol  = "tcp"
    interval  = 5
    timeout   = 2
    threshold = 3
  }

  connection {
    max_connections = 1000
    idle_timeout    = 300000
  }
}

backend "postgres-pool" {
  strategy = "least_conn"

  server {
    address = "postgres-primary.db.local:5432"
    weight  = 10
    role    = "primary"
  }

  server {
    address = "postgres-replica-1.db.local:5432"
    weight  = 5
    role    = "replica"
  }

  health_check {
    enabled   = true
    protocol  = "tcp"
    interval  = 5
    timeout   = 2
    threshold = 3
  }

  connection {
    max_connections = 500
    idle_timeout    = 300000
  }
}

backend "redis-pool" {
  strategy = "round_robin"

  server {
    address = "redis-1.cache.local:6379"
  }

  server {
    address = "redis-2.cache.local:6379"
  }

  server {
    address = "redis-3.cache.local:6379"
  }

  health_check {
    enabled   = true
    protocol  = "tcp"
    interval  = 3
    timeout   = 1
    threshold = 2
  }

  connection {
    max_connections = 5000
    idle_timeout    = 60000
  }
}

connection_pool {
  enabled             = true
  min_idle            = 10
  max_idle            = 100
  max_lifetime        = 3600000
  validation_interval = 30000
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

  database_metrics {
    enabled          = true
    connection_stats = true
    query_stats      = false
  }
}

health {
  enabled  = true
  endpoint = "/health"
  port     = 8081
}
