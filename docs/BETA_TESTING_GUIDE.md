# Beta Testing Guide

Welcome to the Highper Gateway beta testing program! This guide provides comprehensive instructions for testing all 15 use case scenarios with their full feature sets.

## Quick Start

### Prerequisites

- **Rust 1.75+** (for building from source)
- **OpenSSL 1.1+** development libraries
- **Docker** (optional, for running backends)
- **curl with HTTP/3 support** (for scenario 05)
- **grpcurl** (for scenario 07)
- **websocat** (for scenario 06)

### Installation

```bash
# Clone the repository
git clone https://github.com/highperapp/highper-gateway.git
cd highper-gateway

# Build release binary
cargo build --release

# Verify installation
./target/release/highper-gateway --version
```

### Basic Usage

```bash
# Validate a configuration
./target/release/highper-gateway validate -c config.proxy

# Run with a configuration
./target/release/highper-gateway run -c config.proxy

# Run with hot reload enabled
./target/release/highper-gateway run -c config.proxy --watch
```

---

## Test Scenarios Overview

| # | Scenario | Key Features |
|---|----------|--------------|
| 01 | Layer 4 TCP | Raw TCP proxying, connection pooling, protocol detection |
| 02 | Layer 7 HTTP | HTTP/1.1, HTTP/2, reverse proxy, load balancing algorithms |
| 03 | HTTPS/TLS | TLS termination, ACME, mTLS, OCSP stapling |
| 04 | API Gateway | Rate limiting (Token Bucket, Sliding Window), transforms |
| 05 | HTTP/3 QUIC | Cloudflare quiche, 0-RTT, connection migration |
| 06 | WebSocket | Bidirectional, sticky sessions, connection recovery |
| 07 | gRPC | HTTP/2, streaming support, health checks |
| 08 | Database LB | MySQL, PostgreSQL, Redis protocol support |
| 09 | WAF + mTLS | 4 WAF engines, client certificate validation |
| 10 | Hybrid | Multi-protocol on same port |
| 11 | CDN Caching | InMemory, Redis, MultiTier caching |
| 12 | Microservices | Consul, etcd, circuit breaker |
| 13 | GraphQL | Schema stitching, federation |
| 14 | Static + PHP-FPM | FastCGI protocol |
| 15 | Geographic | MaxMind, IP2Location integration |

---

## Scenario 01: Layer 4 TCP Proxying

**Features**: Pure TCP proxying, connection pooling, protocol detection

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-01-layer4-tcp.proxy
```

### Testing

```bash
# Start TCP backends (e.g., echo servers)
nc -l -p 8081 &
nc -l -p 8082 &

# Test TCP connection
echo "Hello TCP" | nc localhost 8080

# Test connection pooling (multiple rapid connections)
for i in {1..10}; do echo "Request $i" | nc localhost 8080; done

# Verify protocol detection in logs
RUST_LOG=debug ./target/release/highper-gateway run -c examples/configs/scenarios/scenario-01-layer4-tcp.proxy
```

### Validation Checklist
- [ ] TCP connections proxied correctly
- [ ] Connection pooling reduces backend connections
- [ ] Protocol detection identifies HTTP, TLS, MySQL, PostgreSQL

---

## Scenario 02: Layer 7 HTTP Load Balancing

**Features**: HTTP/1.1, HTTP/2, reverse proxy, load balancing algorithms (round-robin, least-conn, IP-hash, random, Maglev)

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-02-layer7-http.proxy
```

### Testing

```bash
# Start multiple HTTP backends
docker run -d --name backend1 -p 8081:80 nginx:alpine
docker run -d --name backend2 -p 8082:80 nginx:alpine
docker run -d --name backend3 -p 8083:80 nginx:alpine

# Test HTTP/1.1
curl -v http://localhost:8080/

# Test HTTP/2
curl -v --http2 http://localhost:8080/

# Test load balancing distribution (round-robin)
for i in {1..10}; do curl -s http://localhost:8080/ | grep -o 'backend[0-9]'; done

# Test with different Host headers
curl -H "Host: api.example.com" http://localhost:8080/api/users

# Test health check endpoint
curl http://localhost:8080/health
```

### Load Balancing Algorithms

Test each algorithm by modifying the config:

```yaml
# In config, change lb algorithm:
lb: round_robin    # Cycles through backends
lb: least_conn     # Sends to backend with fewest connections
lb: ip_hash        # Sticky sessions based on client IP
lb: random         # Random selection
lb: maglev         # Consistent hashing (Google Maglev)
```

### Validation Checklist
- [ ] HTTP/1.1 requests proxied correctly
- [ ] HTTP/2 requests proxied correctly
- [ ] Round-robin distributes evenly
- [ ] Least-conn prefers idle backends
- [ ] IP-hash provides sticky sessions
- [ ] Maglev maintains consistency during backend changes

---

## Scenario 03: HTTPS/TLS Termination

**Features**: TLS termination, ACME (Let's Encrypt), mTLS, OCSP stapling, certificate rotation

### Generate Test Certificates

```bash
# Generate CA certificate
openssl genrsa -out ca.key 4096
openssl req -x509 -new -nodes -key ca.key -sha256 -days 365 -out ca.crt \
  -subj "/CN=Test CA"

# Generate server certificate
openssl genrsa -out server.key 2048
openssl req -new -key server.key -out server.csr \
  -subj "/CN=localhost"
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out server.crt -days 365 -sha256

# Generate client certificate (for mTLS)
openssl genrsa -out client.key 2048
openssl req -new -key client.key -out client.csr \
  -subj "/CN=test-client"
openssl x509 -req -in client.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out client.crt -days 365 -sha256
```

### Configuration

Update `scenario-03-layer7-tls.proxy` with certificate paths:

```
https://localhost:8443 {
    tls "server.crt" "server.key"
    proxy http://127.0.0.1:8081
}
```

### Testing TLS Termination

```bash
# Run gateway
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-03-layer7-tls.proxy

# Test HTTPS
curl -v --cacert ca.crt https://localhost:8443/

# Check TLS version
curl -v --cacert ca.crt https://localhost:8443/ 2>&1 | grep "SSL connection"

# Test TLS 1.3 explicitly
curl -v --cacert ca.crt --tlsv1.3 https://localhost:8443/
```

### Testing mTLS (Mutual TLS)

```bash
# Configure mTLS in config:
# mtls ca_file="ca.crt" verify_client=required

# Test with client certificate
curl -v --cacert ca.crt --cert client.crt --key client.key https://localhost:8443/

# Test without client cert (should fail with mTLS enabled)
curl -v --cacert ca.crt https://localhost:8443/
```

### Testing ACME (Let's Encrypt)

```yaml
# Config for ACME:
tls:
  acme:
    email: "admin@example.com"
    directory: "https://acme-staging-v02.api.letsencrypt.org/directory"
    domains:
      - "example.com"
```

### Testing OCSP Stapling

```bash
# Check OCSP response in TLS handshake
openssl s_client -connect localhost:8443 -status < /dev/null 2>&1 | grep -A 20 "OCSP Response"
```

### Validation Checklist
- [ ] TLS 1.2 connections work
- [ ] TLS 1.3 connections work
- [ ] mTLS rejects clients without valid certificates
- [ ] mTLS accepts clients with valid certificates
- [ ] OCSP stapling provides certificate status
- [ ] Certificate hot-reload works (send SIGHUP)

---

## Scenario 04: API Gateway with Rate Limiting

**Features**: Rate limiting (Token Bucket, Sliding Window), request transforms, JWT authentication

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-04-api-gateway.yaml
```

### Testing Token Bucket Rate Limiting

```bash
# Burst of requests (token bucket allows initial burst)
for i in {1..20}; do
  curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8080/api/
done

# Check for 429 Too Many Requests
# First N requests succeed (bucket capacity), then rate-limited
```

### Testing Sliding Window Rate Limiting

```yaml
# Config with sliding window:
rate_limit:
  algorithm: sliding_window
  requests_per_second: 10
  window_size: 1s
```

```bash
# Steady request rate
while true; do
  curl -s -o /dev/null -w "%{http_code} " http://localhost:8080/api/
  sleep 0.05
done
```

### Testing Request Transformation

```bash
# Test header injection
curl -v http://localhost:8080/api/users

# Check response for added headers (X-Request-ID, etc.)
curl -I http://localhost:8080/api/users
```

### Testing JWT Authentication

```bash
# Request without token (should fail)
curl -v http://localhost:8080/api/protected

# Generate test JWT (use jwt.io or similar)
TOKEN="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."

# Request with valid token
curl -v -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/protected
```

### Validation Checklist
- [ ] Token bucket allows initial burst
- [ ] Token bucket enforces sustained rate
- [ ] Sliding window provides smooth rate limiting
- [ ] 429 responses include Retry-After header
- [ ] Request headers transformed correctly
- [ ] JWT authentication validates tokens
- [ ] Invalid JWTs rejected with 401

---

## Scenario 05: HTTP/3 + QUIC

**Features**: Cloudflare quiche implementation, 0-RTT, connection migration, Alt-Svc header

### Prerequisites

```bash
# Install curl with HTTP/3 support
# On Ubuntu:
sudo apt install curl-http3

# Or build from source with quiche support
```

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-05-http3-quic.yaml
```

### Testing HTTP/3

```bash
# Test HTTP/3 connection
curl --http3 -v https://localhost:8443/

# Check Alt-Svc header (advertises HTTP/3)
curl -v https://localhost:8443/ 2>&1 | grep -i "alt-svc"

# Test fallback to HTTP/2
curl --http2 -v https://localhost:8443/
```

### Testing 0-RTT (Early Data)

```bash
# First connection establishes session
curl --http3 https://localhost:8443/

# Subsequent connections can use 0-RTT
curl --http3 -v https://localhost:8443/ 2>&1 | grep "early data"
```

### Testing Connection Migration

```bash
# Monitor QUIC connection IDs in debug logs
RUST_LOG=debug,quiche=debug ./target/release/highper-gateway run -c examples/configs/scenarios/scenario-05-http3-quic.yaml
```

### Validation Checklist
- [ ] HTTP/3 connections established
- [ ] Alt-Svc header advertises h3
- [ ] Fallback to HTTP/2 works
- [ ] 0-RTT reduces handshake latency
- [ ] QUIC metrics exposed in Prometheus

---

## Scenario 06: WebSocket Load Balancer

**Features**: Bidirectional communication, sticky sessions, connection recovery, keep-alive

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-06-websocket.yaml
```

### Start WebSocket Backend

```bash
# Using Python websockets
pip install websockets
python -c "
import asyncio
import websockets

async def echo(websocket, path):
    async for message in websocket:
        await websocket.send(f'Echo: {message}')

asyncio.get_event_loop().run_until_complete(
    websockets.serve(echo, 'localhost', 8081))
asyncio.get_event_loop().run_forever()
"
```

### Testing WebSocket

```bash
# Install websocat
cargo install websocat

# Connect and send messages
websocat ws://localhost:8080/ws

# Type messages and see echoes
> Hello
< Echo: Hello
```

### Testing Sticky Sessions

```bash
# Multiple connections from same IP should go to same backend
websocat ws://localhost:8080/ws &
websocat ws://localhost:8080/ws &

# Check logs for session affinity
RUST_LOG=debug ./target/release/highper-gateway run -c examples/configs/scenarios/scenario-06-websocket.yaml
```

### Testing Connection Recovery

```bash
# Start connection
websocat ws://localhost:8080/ws

# Kill backend, gateway should attempt reconnection
docker stop backend1

# Check recovery logs
```

### Validation Checklist
- [ ] WebSocket upgrade succeeds
- [ ] Bidirectional messages work
- [ ] Sticky sessions maintain affinity
- [ ] Keep-alive pings prevent timeout
- [ ] Connection recovery attempts on backend failure

---

## Scenario 07: gRPC Gateway

**Features**: HTTP/2, unary RPC, streaming (client, server, bidirectional), health checks

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-07-grpc.yaml
```

### Start gRPC Backend

```bash
# Using grpcurl's test server or your own gRPC service
# Example with grpc-health-probe
docker run -d -p 8081:50051 grpc/health-probe-test-server
```

### Testing gRPC

```bash
# Install grpcurl
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest

# List services
grpcurl -plaintext localhost:50051 list

# Call health check
grpcurl -plaintext localhost:50051 grpc.health.v1.Health/Check

# Test unary RPC
grpcurl -plaintext -d '{"name": "World"}' localhost:50051 helloworld.Greeter/SayHello
```

### Testing Streaming

```bash
# Server streaming
grpcurl -plaintext -d '{"count": 5}' localhost:50051 example.StreamService/ServerStream

# Client streaming
grpcurl -plaintext localhost:50051 example.StreamService/ClientStream < requests.json

# Bidirectional streaming
grpcurl -plaintext localhost:50051 example.StreamService/BidiStream
```

### Testing gRPC Health Checks

```bash
# Gateway periodically checks backend health
RUST_LOG=debug ./target/release/highper-gateway run -c examples/configs/scenarios/scenario-07-grpc.yaml 2>&1 | grep "health"
```

### Validation Checklist
- [ ] Unary RPC calls work
- [ ] Server streaming works
- [ ] Client streaming works
- [ ] Bidirectional streaming works
- [ ] gRPC health checks detect unhealthy backends
- [ ] Load balancing across gRPC backends

---

## Scenario 08: Database Load Balancer

**Features**: MySQL, PostgreSQL, Redis protocol support, connection pooling, read/write splitting

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-08-database-lb.yaml
```

### Start Database Backends

```bash
# MySQL
docker run -d --name mysql1 -p 3307:3306 -e MYSQL_ROOT_PASSWORD=test mysql:8
docker run -d --name mysql2 -p 3308:3306 -e MYSQL_ROOT_PASSWORD=test mysql:8

# PostgreSQL
docker run -d --name postgres1 -p 5433:5432 -e POSTGRES_PASSWORD=test postgres:15
docker run -d --name postgres2 -p 5434:5432 -e POSTGRES_PASSWORD=test postgres:15

# Redis
docker run -d --name redis1 -p 6380:6379 redis:7
docker run -d --name redis2 -p 6381:6379 redis:7
```

### Testing MySQL Load Balancing

```bash
# Connect through gateway
mysql -h 127.0.0.1 -P 3306 -u root -ptest

# Run queries and check distribution in logs
mysql> SELECT @@hostname;
```

### Testing PostgreSQL Load Balancing

```bash
# Connect through gateway
psql -h 127.0.0.1 -p 5432 -U postgres

# Check which backend handles query
postgres=# SELECT inet_server_addr();
```

### Testing Redis Load Balancing

```bash
# Connect through gateway
redis-cli -h 127.0.0.1 -p 6379

# Test commands
127.0.0.1:6379> SET key value
127.0.0.1:6379> GET key
```

### Testing Read/Write Splitting

```yaml
# Config for read/write split:
upstreams:
  mysql_write:
    servers: ["mysql1:3306"]
  mysql_read:
    servers: ["mysql2:3306", "mysql3:3306"]
```

### Validation Checklist
- [ ] MySQL connections proxied correctly
- [ ] PostgreSQL connections proxied correctly
- [ ] Redis commands proxied correctly
- [ ] Connection pooling reduces backend connections
- [ ] Protocol detection identifies database type
- [ ] Read/write splitting routes correctly

---

## Scenario 09: WAF + mTLS

**Features**: 4 WAF engines (ModSecurity, Coraza, LibInjection, AWS WAF), OWASP CRS, client certificate validation

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-09-waf-mtls.yaml
```

### Testing WAF - SQL Injection

```bash
# SQL injection attempt (should be blocked)
curl "http://localhost:8080/?id=1' OR '1'='1"
# Expected: 403 Forbidden

curl "http://localhost:8080/?id=1; DROP TABLE users--"
# Expected: 403 Forbidden

# UNION-based injection
curl "http://localhost:8080/?id=1 UNION SELECT * FROM users"
# Expected: 403 Forbidden
```

### Testing WAF - XSS (Cross-Site Scripting)

```bash
# XSS attempt (should be blocked)
curl "http://localhost:8080/?q=<script>alert('xss')</script>"
# Expected: 403 Forbidden

curl "http://localhost:8080/?q=<img src=x onerror=alert('xss')>"
# Expected: 403 Forbidden
```

### Testing WAF - Command Injection

```bash
# Command injection (should be blocked)
curl "http://localhost:8080/?cmd=; cat /etc/passwd"
# Expected: 403 Forbidden

curl "http://localhost:8080/?file=../../../etc/passwd"
# Expected: 403 Forbidden (path traversal)
```

### Testing WAF - Request Size Limits

```bash
# Large body (should be blocked if exceeds limit)
dd if=/dev/zero bs=1M count=10 | curl -X POST --data-binary @- http://localhost:8080/
# Expected: 413 Request Entity Too Large
```

### Testing mTLS with WAF

```bash
# Generate client certificate (see Scenario 03)

# Request with valid client cert
curl --cacert ca.crt --cert client.crt --key client.key https://localhost:8443/

# Request with invalid client cert
curl --cacert ca.crt --cert invalid.crt --key invalid.key https://localhost:8443/
# Expected: TLS handshake failure
```

### WAF Engine Selection

```yaml
# Config to select WAF engine:
waf:
  engine: modsecurity  # or: coraza, libinjection, aws
  ruleset: owasp-crs-4.0
  paranoia_level: 2
```

### Validation Checklist
- [ ] SQL injection blocked
- [ ] XSS attacks blocked
- [ ] Command injection blocked
- [ ] Path traversal blocked
- [ ] Request size limits enforced
- [ ] OWASP CRS rules applied
- [ ] mTLS validates client certificates
- [ ] Invalid certificates rejected

---

## Scenario 10: Hybrid Multi-Protocol

**Features**: Multiple protocols on same port, protocol detection, automatic routing

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-10-hybrid-multiprotocol.yaml
```

### Testing Protocol Detection

```bash
# HTTP request on hybrid port
curl http://localhost:8080/

# HTTPS request on same port
curl --insecure https://localhost:8080/

# WebSocket on same port
websocat ws://localhost:8080/ws

# gRPC on same port
grpcurl -plaintext localhost:8080 list

# Raw TCP on same port
echo "HELLO" | nc localhost 8080
```

### Protocol Routing

```yaml
# Config for protocol-based routing:
routes:
  - match:
      protocol: http
    upstream: http_backends
  - match:
      protocol: grpc
    upstream: grpc_backends
  - match:
      protocol: websocket
    upstream: ws_backends
```

### Validation Checklist
- [ ] HTTP detected and routed correctly
- [ ] HTTPS detected and TLS terminated
- [ ] WebSocket upgrade detected
- [ ] gRPC detected by content-type
- [ ] TCP fallback for unknown protocols

---

## Scenario 11: CDN Edge Caching

**Features**: InMemory cache, Redis cache, MultiTier caching, cache invalidation, conditional requests

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-11-cdn-edge-caching.yaml
```

### Testing Cache Behavior

```bash
# First request (cache miss)
curl -v http://localhost:8080/static/file.js 2>&1 | grep "X-Cache"
# Expected: X-Cache: MISS

# Second request (cache hit)
curl -v http://localhost:8080/static/file.js 2>&1 | grep "X-Cache"
# Expected: X-Cache: HIT

# Check cache headers
curl -I http://localhost:8080/static/file.js
# Expected: Cache-Control, ETag, Last-Modified headers
```

### Testing Conditional Requests

```bash
# Get ETag from first request
ETAG=$(curl -sI http://localhost:8080/static/file.js | grep -i etag | cut -d' ' -f2)

# Conditional request with If-None-Match
curl -v -H "If-None-Match: $ETAG" http://localhost:8080/static/file.js
# Expected: 304 Not Modified
```

### Testing Cache Invalidation

```bash
# Purge specific URL
curl -X PURGE http://localhost:8080/static/file.js

# Purge by pattern
curl -X PURGE "http://localhost:8080/static/*"

# Verify cache miss after purge
curl -v http://localhost:8080/static/file.js 2>&1 | grep "X-Cache"
# Expected: X-Cache: MISS
```

### Cache Backend Configuration

```yaml
# InMemory cache:
cache:
  backend: memory
  max_size: 100MB
  ttl: 3600

# Redis cache:
cache:
  backend: redis
  url: "redis://localhost:6379"
  ttl: 3600

# MultiTier (memory + Redis):
cache:
  backend: multi_tier
  tiers:
    - type: memory
      max_size: 50MB
    - type: redis
      url: "redis://localhost:6379"
```

### Validation Checklist
- [ ] Cache miss on first request
- [ ] Cache hit on subsequent requests
- [ ] ETag/Last-Modified headers set
- [ ] Conditional requests return 304
- [ ] Cache purge invalidates entries
- [ ] TTL expiration works
- [ ] MultiTier falls through correctly

---

## Scenario 12: Microservices Discovery

**Features**: Consul integration, etcd integration, Kubernetes service discovery, circuit breaker

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-12-microservices-discovery.proxy
```

### Start Consul

```bash
# Run Consul in dev mode
docker run -d --name consul -p 8500:8500 consul:latest agent -dev -client=0.0.0.0

# Register a service
curl -X PUT -d '{"ID": "backend1", "Name": "api", "Address": "127.0.0.1", "Port": 8081}' \
  http://localhost:8500/v1/agent/service/register

curl -X PUT -d '{"ID": "backend2", "Name": "api", "Address": "127.0.0.1", "Port": 8082}' \
  http://localhost:8500/v1/agent/service/register
```

### Start etcd

```bash
# Run etcd
docker run -d --name etcd -p 2379:2379 \
  quay.io/coreos/etcd:v3.5.0 \
  /usr/local/bin/etcd --listen-client-urls http://0.0.0.0:2379 --advertise-client-urls http://localhost:2379

# Register services
etcdctl put /services/api/backend1 '{"address": "127.0.0.1", "port": 8081}'
etcdctl put /services/api/backend2 '{"address": "127.0.0.1", "port": 8082}'
```

### Discovery Configuration

```yaml
# Consul discovery:
discovery:
  type: consul
  consul:
    address: "http://localhost:8500"
    service: "api"
    health_check_interval: 10s

# etcd discovery:
discovery:
  type: etcd
  etcd:
    endpoints: ["http://localhost:2379"]
    prefix: "/services/api"

# Kubernetes discovery:
discovery:
  type: kubernetes
  kubernetes:
    namespace: "default"
    label_selector: "app=api"
```

### Testing Service Discovery

```bash
# Add new backend to Consul
curl -X PUT -d '{"ID": "backend3", "Name": "api", "Address": "127.0.0.1", "Port": 8083}' \
  http://localhost:8500/v1/agent/service/register

# Gateway should automatically discover and route to new backend
curl http://localhost:8080/api/

# Check discovered backends
curl http://localhost:9000/api/backends
```

### Testing Circuit Breaker

```bash
# Stop a backend to trigger circuit breaker
docker stop backend1

# Make requests - circuit should open after failures
for i in {1..20}; do
  curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8080/api/
done

# Check circuit breaker state in metrics
curl http://localhost:9090/metrics | grep circuit
```

### Validation Checklist
- [ ] Consul service discovery works
- [ ] etcd service discovery works
- [ ] New backends auto-discovered
- [ ] Removed backends auto-removed
- [ ] Circuit breaker opens on failures
- [ ] Circuit breaker closes after recovery
- [ ] Health checks update backend status

---

## Scenario 13: GraphQL Gateway

**Features**: Schema stitching, federation, query routing, introspection

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-13-graphql.proxy
```

### Start GraphQL Backends

```bash
# Start multiple GraphQL services
docker run -d --name graphql1 -p 4001:4000 your-graphql-service
docker run -d --name graphql2 -p 4002:4000 your-graphql-service
```

### Testing GraphQL Queries

```bash
# Simple query
curl -X POST http://localhost:8080/graphql \
  -H "Content-Type: application/json" \
  -d '{"query": "{ users { id name } }"}'

# Query with variables
curl -X POST http://localhost:8080/graphql \
  -H "Content-Type: application/json" \
  -d '{"query": "query GetUser($id: ID!) { user(id: $id) { name } }", "variables": {"id": "1"}}'

# Introspection query
curl -X POST http://localhost:8080/graphql \
  -H "Content-Type: application/json" \
  -d '{"query": "{ __schema { types { name } } }"}'
```

### Testing Schema Stitching

```yaml
# Config for schema stitching:
graphql:
  schema_stitching:
    services:
      - name: users
        url: "http://localhost:4001/graphql"
        schema_path: "users.graphql"
      - name: orders
        url: "http://localhost:4002/graphql"
        schema_path: "orders.graphql"
```

```bash
# Query across stitched schemas
curl -X POST http://localhost:8080/graphql \
  -H "Content-Type: application/json" \
  -d '{"query": "{ user(id: 1) { name orders { id total } } }"}'
```

### Testing Federation

```yaml
# Config for Apollo Federation:
graphql:
  federation:
    supergraph_sdl: "supergraph.graphql"
    subgraphs:
      - name: users
        url: "http://localhost:4001/graphql"
      - name: products
        url: "http://localhost:4002/graphql"
```

### Validation Checklist
- [ ] Simple queries routed correctly
- [ ] Mutations work
- [ ] Variables resolved
- [ ] Introspection returns schema
- [ ] Schema stitching merges schemas
- [ ] Federation resolves entities
- [ ] Query depth limiting works

---

## Scenario 14: Static Files + PHP-FPM

**Features**: Static file serving, FastCGI protocol, PHP-FPM integration, MIME types

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-14-static-php-fpm.yaml
```

### Start PHP-FPM

```bash
# Run PHP-FPM container
docker run -d --name php-fpm -p 9000:9000 \
  -v $(pwd)/www:/var/www/html \
  php:8.2-fpm

# Create test PHP file
mkdir -p www
echo '<?php phpinfo();' > www/info.php
echo '<?php echo json_encode($_GET);' > www/api.php
```

### Testing Static Files

```bash
# Create static files
mkdir -p www/static
echo "body { color: blue; }" > www/static/style.css
echo "console.log('hello');" > www/static/script.js

# Request static files
curl http://localhost:8080/static/style.css
curl http://localhost:8080/static/script.js

# Check MIME types
curl -I http://localhost:8080/static/style.css | grep Content-Type
# Expected: Content-Type: text/css

curl -I http://localhost:8080/static/script.js | grep Content-Type
# Expected: Content-Type: application/javascript
```

### Testing PHP-FPM

```bash
# PHP info page
curl http://localhost:8080/info.php

# PHP with query parameters
curl "http://localhost:8080/api.php?name=test&value=123"

# PHP POST request
curl -X POST -d "name=test" http://localhost:8080/api.php
```

### FastCGI Configuration

```yaml
# Config for PHP-FPM:
routes:
  - match:
      path_suffix: ".php"
    fastcgi:
      address: "127.0.0.1:9000"
      script_filename: "/var/www/html$uri"
      params:
        SCRIPT_FILENAME: "/var/www/html$uri"
        DOCUMENT_ROOT: "/var/www/html"
```

### Validation Checklist
- [ ] Static files served correctly
- [ ] Correct MIME types returned
- [ ] PHP files executed via FastCGI
- [ ] Query parameters passed to PHP
- [ ] POST data passed to PHP
- [ ] PHP errors handled gracefully
- [ ] Directory index (index.php) works

---

## Scenario 15: Geographic Load Balancing

**Features**: MaxMind GeoIP, IP2Location, latency-based routing, geographic affinity

### Configuration

```bash
./target/release/highper-gateway run -c examples/configs/scenarios/scenario-15-geo-routing.proxy
```

### Setup GeoIP Database

```bash
# Download MaxMind GeoLite2 database (requires free account)
# https://dev.maxmind.com/geoip/geolite2-free-geolocation-data

# Place database file
mkdir -p data
mv GeoLite2-City.mmdb data/
```

### Geographic Routing Configuration

```yaml
# Config for geo-routing:
geo:
  database: "data/GeoLite2-City.mmdb"
  provider: maxmind  # or: ip2location

upstreams:
  us_east:
    servers: ["us-east-1.example.com:8080"]
    location: { lat: 39.0, lon: -77.0 }
  us_west:
    servers: ["us-west-1.example.com:8080"]
    location: { lat: 37.0, lon: -122.0 }
  eu_west:
    servers: ["eu-west-1.example.com:8080"]
    location: { lat: 53.0, lon: -8.0 }

routes:
  - match:
      path: "/*"
    load_balancing:
      algorithm: geographic
      fallback: us_east
```

### Testing Geographic Routing

```bash
# Test with X-Forwarded-For header (simulating different locations)

# US East coast IP
curl -H "X-Forwarded-For: 72.21.206.80" http://localhost:8080/

# US West coast IP
curl -H "X-Forwarded-For: 199.87.154.255" http://localhost:8080/

# European IP
curl -H "X-Forwarded-For: 82.132.248.70" http://localhost:8080/

# Check which backend was selected in logs
RUST_LOG=debug ./target/release/highper-gateway run -c examples/configs/scenarios/scenario-15-geo-routing.proxy
```

### Testing Latency-Based Routing

```yaml
# Config for latency-based:
load_balancing:
  algorithm: latency
  health_check_interval: 10s
```

```bash
# Gateway measures latency to backends and routes to fastest
curl http://localhost:8080/

# Check latency metrics
curl http://localhost:9090/metrics | grep backend_latency
```

### Validation Checklist
- [ ] MaxMind database loads correctly
- [ ] IP geolocation returns correct country/region
- [ ] Requests routed to nearest backend
- [ ] Fallback works when geo lookup fails
- [ ] Latency-based routing selects fastest backend
- [ ] X-Forwarded-For header respected

---

## Monitoring and Metrics

### Prometheus Metrics

```bash
# All scenarios expose metrics on port 9090
curl http://localhost:9090/metrics

# Key metrics to monitor:
# - highper_requests_total
# - highper_request_duration_seconds
# - highper_backend_health
# - highper_cache_hits_total
# - highper_circuit_breaker_state
```

### Admin Dashboard

```bash
# Health check
curl http://localhost:9000/api/health

# Statistics
curl http://localhost:9000/api/stats

# Backend status
curl http://localhost:9000/api/backends

# Cache stats
curl http://localhost:9000/api/cache/stats
```

### Debug Logging

```bash
# Enable all debug logs
RUST_LOG=debug ./target/release/highper-gateway run -c config.proxy

# Enable specific module logs
RUST_LOG=highper_gateway::proxy=debug,highper_gateway::cache=info ./target/release/highper-gateway run -c config.proxy

# Save logs to file
RUST_LOG=debug ./target/release/highper-gateway run -c config.proxy 2>&1 | tee gateway.log
```

---

## Performance Testing

### Using wrk

```bash
wrk -t12 -c400 -d30s http://localhost:8080/
wrk -t12 -c400 -d30s -s scripts/post.lua http://localhost:8080/api/
```

### Using k6

```bash
k6 run --vus 100 --duration 30s scripts/load_test.js
```

### Using oha (Rust-based)

```bash
oha -n 100000 -c 500 http://localhost:8080/
```

---

## Reporting Issues

When reporting issues, please include:

1. **Scenario number** and configuration file
2. **Steps to reproduce**
3. **Expected vs actual behavior**
4. **Logs** (`RUST_LOG=debug`)
5. **Environment** (OS, Rust version, Docker version)

### Create Issue Template

```markdown
**Scenario**: #XX - Name
**Config file**: examples/configs/scenarios/scenario-XX-name.yaml

**Steps to reproduce**:
1. Start gateway with config
2. Run command X
3. Observe behavior Y

**Expected**: ...
**Actual**: ...

**Logs**: (attach gateway.log)
**Environment**: Linux/macOS/Windows, Rust 1.XX, Docker XX
```

---

## Getting Help

- **Documentation**: See `/docs/` folder
- **Issues**: GitHub Issues
- **Discussions**: GitHub Discussions

Thank you for participating in the beta test!
