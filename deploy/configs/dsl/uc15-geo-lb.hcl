# Highper Gateway DSL Configuration
# Use Case 15: Geographic Load Balancing

server {
  name = "highper-gateway-geo-lb"
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

listener "http-geo" {
  protocol = "http"
  bind     = "0.0.0.0:80"

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

listener "https-geo" {
  protocol = "https"
  bind     = "0.0.0.0:443"
  tls      = "default"

  route {
    match {
      path_prefix = "/"
    }
    backend = "geo-routing"
    geo {
      enabled = true
    }
  }
}

backend "geo-routing" {
  strategy = "geo"

  geo {
    database       = "/etc/highper-gateway/geoip/GeoLite2-City.mmdb"
    default_region = "us-east"
  }

  region "us-east" {
    countries = ["US", "CA"]
    states {
      US = ["NY", "NJ", "PA", "MA", "CT"]
    }

    server {
      address = "us-east-1.app.global:8080"
    }

    server {
      address = "us-east-2.app.global:8080"
    }
  }

  region "us-west" {
    countries = ["US"]
    states {
      US = ["CA", "WA", "OR", "NV", "AZ"]
    }

    server {
      address = "us-west-1.app.global:8080"
    }

    server {
      address = "us-west-2.app.global:8080"
    }
  }

  region "eu-west" {
    countries = ["GB", "IE", "FR", "ES", "PT", "NL", "BE"]

    server {
      address = "eu-west-1.app.global:8080"
    }

    server {
      address = "eu-west-2.app.global:8080"
    }
  }

  region "eu-central" {
    countries = ["DE", "AT", "CH", "PL", "CZ", "IT"]

    server {
      address = "eu-central-1.app.global:8080"
    }

    server {
      address = "eu-central-2.app.global:8080"
    }
  }

  region "asia-pacific" {
    countries = ["JP", "KR", "SG", "AU", "NZ", "IN"]

    server {
      address = "ap-1.app.global:8080"
    }

    server {
      address = "ap-2.app.global:8080"
    }
  }

  region "latam" {
    countries = ["BR", "MX", "AR", "CL", "CO"]

    server {
      address = "latam-1.app.global:8080"
    }
  }

  health_check {
    enabled  = true
    protocol = "http"
    path     = "/health"
    interval = 10
  }

  failover {
    enabled   = true
    threshold = 2

    fallback_regions {
      us-east      = ["us-west", "eu-west"]
      us-west      = ["us-east", "asia-pacific"]
      eu-west      = ["eu-central", "us-east"]
      eu-central   = ["eu-west", "us-east"]
      asia-pacific = ["us-west", "eu-central"]
      latam        = ["us-east", "us-west"]
    }
  }
}

geoip {
  database = "/etc/highper-gateway/geoip/GeoLite2-City.mmdb"

  auto_update {
    enabled         = true
    interval        = 604800  # Weekly
    license_key_env = "MAXMIND_LICENSE_KEY"
  }
}

latency_routing {
  enabled        = true
  check_interval = 60
  samples        = 5
  weight_factor  = 0.7  # 70% geo, 30% latency
}

headers {
  response {
    add {
      name  = "X-Geo-Region"
      value = "$geo_region"
    }

    add {
      name  = "X-Geo-Country"
      value = "$geo_country"
    }

    add {
      name  = "X-Server-Location"
      value = "$upstream_location"
    }
  }
}

logging {
  level  = "info"
  format = "json"
  output = "/var/log/highper-gateway/gateway.log"

  geo_logging {
    enabled     = true
    log_country = true
    log_region  = true
  }
}

metrics {
  enabled  = true
  endpoint = "/metrics"
  port     = 9090

  geo_metrics {
    enabled    = true
    by_region  = true
    by_country = true
  }
}

health {
  enabled  = true
  endpoint = "/health"
  port     = 8081
}
