# HTTP/HTTPS API Gateway Configuration (DSL Format)
#
# This is a Caddy-like DSL configuration for an HTTP API gateway.
# It demonstrates the simplified syntax alternative to YAML.
#
# Environment variables:
# - API_BACKEND_1: First API server
# - API_BACKEND_2: Second API server
# - TLS_CERT_PATH: Path to TLS certificate
# - TLS_KEY_PATH: Path to TLS private key

# Global configuration
{
    # Logging configuration
    logging {
        format json
        level info
        output stdout

        # Protocol-specific log levels
        protocols {
            http info
            tls info
        }
    }

    # Admin API
    admin {
        bind 127.0.0.1:9000
        auth true
        api_key ${ADMIN_API_KEY}
    }

    # Metrics
    metrics {
        bind 0.0.0.0:9090
        endpoint /metrics
    }

    # Tracing
    tracing {
        enabled true
        endpoint http://jaeger:14268/api/traces
        sample_rate 0.1
    }
}

# API gateway site
api.example.com {
    # Listen on HTTP and HTTPS
    listen 0.0.0.0:8080
    listen 0.0.0.0:8443 {
        tls ${TLS_CERT_PATH} ${TLS_KEY_PATH}
        tls_min_version 1.2
    }

    # Protocols
    protocols http1 http2

    # Performance tuning
    performance {
        max_connections 100000
        connect_timeout 5s
        request_timeout 30s
        idle_timeout 90s

        connection_pool {
            max_idle_per_host 100
            metrics true
        }
    }

    # Upstream backend pool
    upstream api-backend {
        server ${API_BACKEND_1} weight=1
        server ${API_BACKEND_2} weight=1

        load_balancing round_robin

        health_check {
            interval 10s
            timeout 3s
            unhealthy_threshold 3
            healthy_threshold 2
            http /health 200
        }
    }

    # Routes
    route /api/v1 {
        methods GET POST PUT DELETE PATCH
        proxy_to api-backend

        # Middlewares
        cors {
            origins *
            methods GET POST PUT DELETE PATCH
            headers *
        }

        rate_limit {
            algorithm token_bucket
            capacity 1000
            refill_rate 100
            window 60s
            key_type ip
        }

        security_headers {
            x_content_type_options nosniff
            x_frame_options DENY
            x_xss_protection "1; mode=block"
            hsts "max-age=31536000; includeSubDomains"
            referrer_policy strict-origin-when-cross-origin
        }

        cache {
            ttl 300s
            max_size 10000
            methods GET HEAD
            key_headers Authorization
        }
    }

    # Default route
    route / {
        proxy_to api-backend

        security_headers {
            preset strict
        }
    }
}
