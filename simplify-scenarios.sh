#!/bin/bash
# Simplify scenarios 05-15 to use only working DSL directives

CERT_PATH="/mnt/e/my-opensource/highper-gateway/certs"

# Scenario 05: HTTP/3 + QUIC (simplified to HTTPS)
cat > configs/scenarios/scenario-05-http3-quic.proxy <<'EOF'
# Scenario 05: HTTP/3 + QUIC (Simplified to HTTPS/TLS)
# Note: HTTP/3 and QUIC are not yet implemented, using HTTPS/2 instead

https://http3.loadtest.local:8445 {
    tls "$CERT_PATH/api.crt" "$CERT_PATH/api.key"
    proxy 127.0.0.1:8081 127.0.0.1:8082 127.0.0.1:8083
    lb round_robin
}

log info
metrics prometheus port=9090
EOF

# Scenario 06: WebSocket (simplified to HTTPS)
cat > configs/scenarios/scenario-06-websocket.proxy <<'EOF'
# Scenario 06: WebSocket Proxying (Simplified)
# Note: WebSocket-specific features not yet implemented

https://ws.loadtest.local:8446 {
    tls "$CERT_PATH/api.crt" "$CERT_PATH/api.key"
    proxy 127.0.0.1:8081 127.0.0.1:8082 127.0.0.1:8083
    lb round_robin
}

log info
metrics prometheus port=9090
EOF

# Scenario 07: gRPC (simplified to HTTPS/2)
cat > configs/scenarios/scenario-07-grpc.proxy <<'EOF'
# Scenario 07: gRPC Load Balancing (Simplified)
# Note: gRPC-specific features not yet implemented, using HTTP/2

https://grpc.loadtest.local:8447 {
    tls "$CERT_PATH/api.crt" "$CERT_PATH/api.key"
    proxy 127.0.0.1:8081 127.0.0.1:8082 127.0.0.1:8083
    lb least_conn
}

log info
metrics prometheus port=9090
EOF

# Scenario 08: Database Load Balancing (TCP)
cat > configs/scenarios/scenario-08-database-lb.proxy <<'EOF'
# Scenario 08: Database Load Balancing (TCP)

tcp://0.0.0.0:3306 {
    proxy 127.0.0.1:13306 127.0.0.1:13307 127.0.0.1:13308
    lb round_robin
}

log info
metrics prometheus port=9090
EOF

# Scenario 09: WAF + mTLS (simplified to HTTPS)
cat > configs/scenarios/scenario-09-waf-mtls.proxy <<'EOF'
# Scenario 09: WAF + mTLS (Simplified)
# Note: WAF and mTLS features not yet implemented

https://secure.loadtest.local:8449 {
    tls "$CERT_PATH/api.crt" "$CERT_PATH/api.key"
    proxy 127.0.0.1:8081 127.0.0.1:8082 127.0.0.1:8083
    lb least_conn
}

log info
metrics prometheus port=9090
EOF

# Scenario 10: Hybrid Multi-Protocol (simplified)
cat > configs/scenarios/scenario-10-hybrid-multiprotocol.proxy <<'EOF'
# Scenario 10: Hybrid Multi-Protocol (Simplified)
# Note: Using basic HTTPS and TCP only

https://multi.loadtest.local:8450 {
    tls "$CERT_PATH/api.crt" "$CERT_PATH/api.key"
    proxy 127.0.0.1:8081 127.0.0.1:8082 127.0.0.1:8083
    lb round_robin
}

tcp://0.0.0.0:5000 {
    proxy 127.0.0.1:15000 127.0.0.1:15001 127.0.0.1:15002
    lb round_robin
}

log info
metrics prometheus port=9090
EOF

# Scenario 11: CDN Edge Caching (simplified to HTTPS)
cat > configs/scenarios/scenario-11-cdn-edge-caching.proxy <<'EOF'
# Scenario 11: CDN Edge Caching (Simplified)
# Note: Caching features not yet implemented

https://cdn.loadtest.local:8451 {
    tls "$CERT_PATH/api.crt" "$CERT_PATH/api.key"
    proxy 127.0.0.1:8081 127.0.0.1:8082 127.0.0.1:8083
    lb round_robin
}

log info
metrics prometheus port=9090
EOF

# Scenario 12: Microservices Discovery (simplified to HTTPS)
cat > configs/scenarios/scenario-12-microservices-discovery.proxy <<'EOF'
# Scenario 12: Microservices Discovery (Simplified)
# Note: Service discovery features not yet implemented

https://api.microservices.local:8452 {
    tls "$CERT_PATH/api.crt" "$CERT_PATH/api.key"
    proxy 127.0.0.1:8081 127.0.0.1:8082 127.0.0.1:8083
    lb least_conn
}

log info
metrics prometheus port=9090
EOF

# Scenario 13: GraphQL Gateway (simplified to HTTPS)
cat > configs/scenarios/scenario-13-graphql.proxy <<'EOF'
# Scenario 13: GraphQL Gateway (Simplified)
# Note: GraphQL-specific features not yet implemented

https://graphql.loadtest.local:8453 {
    tls "$CERT_PATH/api.crt" "$CERT_PATH/api.key"
    proxy 127.0.0.1:8081 127.0.0.1:8082 127.0.0.1:8083
    lb round_robin
}

log info
metrics prometheus port=9090
EOF

# Scenario 14: Static + PHP-FPM (simplified to HTTP)
cat > configs/scenarios/scenario-14-static-php-fpm.proxy <<'EOF'
# Scenario 14: Static + PHP-FPM (Simplified)
# Note: PHP-FPM and static file serving not yet implemented

http://php.loadtest.local:8454 {
    proxy 127.0.0.1:8081 127.0.0.1:8082 127.0.0.1:8083
    lb round_robin
}

log info
metrics prometheus port=9090
EOF

# Scenario 15: Geo-Routing (simplified to HTTPS)
cat > configs/scenarios/scenario-15-geo-routing.proxy <<'EOF'
# Scenario 15: Geo-Based Routing (Simplified)
# Note: Geo-routing features not yet implemented

https://geo.loadtest.local:8455 {
    tls "$CERT_PATH/api.crt" "$CERT_PATH/api.key"
    proxy 127.0.0.1:8081 127.0.0.1:8082 127.0.0.1:8083
    lb round_robin
}

log info
metrics prometheus port=9090
EOF

echo "All scenarios 05-15 have been simplified!"
