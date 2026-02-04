# Highper Gateway - Manual Code Review Checklist

This document provides a comprehensive checklist for manual expert code review, organized by the 15 supported use case scenarios.

**Version:** 1.1.0
**Last Updated:** January 2026

---

## How to Use This Checklist

1. **Before Review:** Run `./scripts/code-review.sh --full` for automated checks
2. **During Review:** Work through scenario-specific sections below
3. **After Review:** Document findings in GitHub Issues with label `code-review`

---

## General Review Checklist (All Scenarios)

### Memory Safety
- [ ] No unbounded allocations (Vec growth, String concatenation)
- [ ] Buffer sizes validated before use
- [ ] No memory leaks in error paths
- [ ] Arc/Rc cycles avoided or documented
- [ ] Unsafe blocks justified and audited

### Concurrency
- [ ] No deadlock potential (lock ordering documented)
- [ ] Atomic operations use appropriate memory ordering
- [ ] Channel bounds prevent unbounded memory growth
- [ ] Async tasks have proper cancellation handling
- [ ] No blocking operations in async context

### Error Handling
- [ ] All errors propagated or logged (no silent failures)
- [ ] Error messages don't leak sensitive information
- [ ] Panic paths documented and justified
- [ ] Recovery possible from transient failures

### Input Validation
- [ ] All external input validated before use
- [ ] Size limits enforced (headers, body, paths)
- [ ] Encoding validated (UTF-8, base64, etc.)
- [ ] Injection attacks prevented (SQL, XSS, command)

---

## Scenario 01: Layer 4 TCP Proxy

**Source Files:**
- `highper-gateway/src/tcp/`
- `highper-gateway/src/proxy/server.rs`

### Protocol Detection
- [ ] Protocol detection timeout prevents hanging
- [ ] Unknown protocols handled gracefully
- [ ] Detection doesn't consume too much memory
- [ ] TLS ClientHello correctly identified

### Connection Management
- [ ] Connection limits enforced per client IP
- [ ] Idle connections cleaned up
- [ ] Half-open connection timeout
- [ ] Backpressure when backend slow

### Buffer Handling
- [ ] Zero-copy where possible (splice/sendfile)
- [ ] Buffer pool prevents allocation churn
- [ ] Large transfers don't OOM
- [ ] Partial reads/writes handled correctly

### Test Cases
```bash
# TCP proxy smoke test
echo "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n" | nc localhost 8080

# Connection limit test
for i in {1..1000}; do nc -w 1 localhost 8080 & done
```

---

## Scenario 02: Layer 7 HTTP Load Balancing

**Source Files:**
- `highper-gateway/src/proxy/handler.rs`
- `highper-gateway/src/proxy/loadbalancer.rs`

### Request Parsing
- [ ] HTTP/1.1 and HTTP/2 supported
- [ ] Invalid requests rejected with proper status
- [ ] Header size limits enforced
- [ ] Chunked encoding handled correctly
- [ ] HTTP smuggling prevented (CL/TE conflicts)

### Load Balancing
- [ ] Round-robin distributes evenly
- [ ] Least-connections tracks accurately
- [ ] IP-hash consistent across restarts
- [ ] Maglev handles backend changes gracefully
- [ ] Weight respected in distribution

### Health Checking
- [ ] Unhealthy backends removed from rotation
- [ ] Recovery detection works
- [ ] Health check doesn't block request path
- [ ] Passive health (response codes) works

### Test Cases
```bash
# Load balancing distribution
for i in {1..100}; do curl -s localhost:8080 | grep backend; done | sort | uniq -c

# HTTP smuggling test
printf "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 6\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\nG" | nc localhost 8080
```

---

## Scenario 03: HTTPS/TLS Termination

**Source Files:**
- `highper-gateway/src/tls/`

### Certificate Management
- [ ] Certificate chain validated correctly
- [ ] Expired certificates rejected
- [ ] Revoked certificates checked (OCSP/CRL)
- [ ] Private key protected (file permissions)
- [ ] Certificate hot-reload works

### TLS Configuration
- [ ] TLS 1.2 minimum enforced
- [ ] Strong cipher suites only
- [ ] ALPN negotiation correct (h2, http/1.1)
- [ ] SNI routing works
- [ ] Session resumption secure

### mTLS
- [ ] Client certificate verified
- [ ] Certificate DN extracted correctly
- [ ] Untrusted clients rejected
- [ ] CA trust store configurable

### ACME/Let's Encrypt
- [ ] HTTP-01 challenge works
- [ ] Renewal before expiry
- [ ] Error handling on ACME failure
- [ ] Rate limits respected

### Test Cases
```bash
# TLS version test
openssl s_client -connect localhost:8443 -tls1_2
openssl s_client -connect localhost:8443 -tls1_3

# OCSP stapling
openssl s_client -connect localhost:8443 -status

# mTLS test
curl --cert client.crt --key client.key https://localhost:8443/
```

---

## Scenario 04: API Gateway with Rate Limiting

**Source Files:**
- `highper-gateway/src/middleware/rate_limit.rs`
- `highper-gateway/src/gateway/auth/`

### Rate Limiting
- [ ] Token bucket refills correctly
- [ ] Sliding window accurate at boundaries
- [ ] Per-IP and per-API-key limits
- [ ] Distributed rate limiting (Redis backend)
- [ ] 429 includes Retry-After header

### Authentication
- [ ] JWT signature verified
- [ ] JWT expiration checked
- [ ] API key lookup constant-time
- [ ] OAuth2 token validation
- [ ] Auth bypass impossible

### Request Transformation
- [ ] Headers added/removed correctly
- [ ] Body transformation doesn't corrupt
- [ ] Query parameters preserved
- [ ] Path rewriting correct

### Test Cases
```bash
# Rate limit test
for i in {1..100}; do curl -s -o /dev/null -w "%{http_code}\n" localhost:8080/api/; done | grep 429 | wc -l

# JWT validation
curl -H "Authorization: Bearer invalid" localhost:8080/api/protected
curl -H "Authorization: Bearer $(./gen-jwt.sh)" localhost:8080/api/protected
```

---

## Scenario 05: HTTP/3 QUIC

**Source Files:**
- `highper-gateway/src/http/http3_quiche.rs`
- `highper-gateway/src/http/alt_svc.rs`

### QUIC Protocol
- [ ] Connection migration supported
- [ ] 0-RTT replay protection
- [ ] Packet encryption correct
- [ ] Flow control enforced
- [ ] Congestion control working

### HTTP/3 Specifics
- [ ] QPACK header compression
- [ ] Stream prioritization
- [ ] Server push (if supported)
- [ ] Graceful fallback to HTTP/2

### Security
- [ ] QUIC version negotiation
- [ ] Connection ID rotation
- [ ] Amplification attack protection
- [ ] Invalid frame handling

### Test Cases
```bash
# HTTP/3 connection
curl --http3 https://localhost:8443/

# Alt-Svc header
curl -v https://localhost:8443/ 2>&1 | grep -i alt-svc

# Fallback test
curl --http3 --http2 https://localhost:8443/
```

---

## Scenario 06: WebSocket Load Balancer

**Source Files:**
- `highper-gateway/src/websocket/`

### Connection Handling
- [ ] Upgrade handshake correct
- [ ] Sec-WebSocket-Key validated
- [ ] Protocol negotiation works
- [ ] Connection limit enforced

### Message Handling
- [ ] Binary and text frames supported
- [ ] Fragmented messages reassembled
- [ ] Large messages handled (streaming)
- [ ] Invalid frames rejected
- [ ] Masking enforced (client → server)

### Session Management
- [ ] Sticky sessions work
- [ ] Session recovery on backend failure
- [ ] Clean disconnect propagated
- [ ] Ping/pong keepalive working

### Test Cases
```bash
# WebSocket upgrade
websocat ws://localhost:8080/ws

# Large message test
dd if=/dev/urandom bs=1M count=1 | websocat -b ws://localhost:8080/ws

# Sticky session test
for i in {1..10}; do echo "test" | websocat ws://localhost:8080/ws; done
```

---

## Scenario 07: gRPC Gateway

**Source Files:**
- `highper-gateway/src/grpc/`

### gRPC Protocol
- [ ] Unary RPC works
- [ ] Server streaming works
- [ ] Client streaming works
- [ ] Bidirectional streaming works
- [ ] Metadata forwarded correctly

### Error Handling
- [ ] gRPC status codes mapped correctly
- [ ] Deadline propagation
- [ ] Cancellation propagated
- [ ] Partial failures handled

### Health Checking
- [ ] gRPC health protocol supported
- [ ] Per-service health status
- [ ] Health affects routing

### Test Cases
```bash
# gRPC health check
grpcurl -plaintext localhost:50051 grpc.health.v1.Health/Check

# Unary call
grpcurl -plaintext -d '{"name": "test"}' localhost:50051 api.Service/Method

# Streaming
grpcurl -plaintext localhost:50051 api.Service/StreamMethod
```

---

## Scenario 08: Database Load Balancer

**Source Files:**
- `highper-gateway/src/tcp/`
- `highper-gateway/src/proxy/database_pool.rs`

### Protocol Support
- [ ] MySQL protocol parsing correct
- [ ] PostgreSQL protocol parsing correct
- [ ] Redis protocol parsing correct
- [ ] Connection state tracked

### Security
- [ ] Credentials not logged
- [ ] TLS to backend supported
- [ ] Query logging optional
- [ ] Connection stealing prevented

### Connection Pooling
- [ ] Pool size limits enforced
- [ ] Connection reuse works
- [ ] Broken connections detected
- [ ] Connection age limits

### Test Cases
```bash
# MySQL through gateway
mysql -h localhost -P 3306 -u user -p

# PostgreSQL through gateway
psql -h localhost -p 5432 -U user

# Redis through gateway
redis-cli -h localhost -p 6379
```

---

## Scenario 09: WAF + mTLS

**Source Files:**
- `highper-gateway/src/middleware/waf/`
- `highper-gateway/src/tls/client_verifier.rs`

### WAF Rules
- [ ] SQL injection detected
- [ ] XSS detected
- [ ] Command injection detected
- [ ] Path traversal detected
- [ ] Request size limits enforced
- [ ] OWASP CRS rules loaded

### WAF Engines
- [ ] ModSecurity engine works
- [ ] Coraza engine works
- [ ] LibInjection works
- [ ] AWS WAF integration works

### Bypass Prevention
- [ ] Encoding bypasses blocked (URL, Unicode)
- [ ] Case variations detected
- [ ] Comment injection detected
- [ ] HTTP parameter pollution

### Test Cases
```bash
# SQL injection
curl "localhost:8080/?id=1' OR '1'='1"
curl "localhost:8080/?id=1; DROP TABLE users--"

# XSS
curl "localhost:8080/?q=<script>alert(1)</script>"

# Path traversal
curl "localhost:8080/?file=../../../etc/passwd"

# Large request
dd if=/dev/zero bs=1M count=20 | curl -X POST --data-binary @- localhost:8080/
```

---

## Scenario 10: Hybrid Multi-Protocol

**Source Files:**
- `highper-gateway/src/tcp/protocol_detect.rs`

### Protocol Detection
- [ ] HTTP detected by "GET/POST/etc"
- [ ] TLS detected by ClientHello
- [ ] gRPC detected by content-type
- [ ] WebSocket detected by Upgrade
- [ ] MySQL/PostgreSQL detected by handshake

### Routing
- [ ] Correct backend per protocol
- [ ] Unknown protocol fallback
- [ ] Detection doesn't block too long

### Resource Management
- [ ] Single port shared efficiently
- [ ] Memory usage reasonable
- [ ] No protocol confusion attacks

---

## Scenario 11: CDN Edge Caching

**Source Files:**
- `highper-gateway/src/cache/`
- `highper-gateway/src/gateway/cache/`

### Cache Behavior
- [ ] Cache key generation correct
- [ ] Vary header respected
- [ ] Cache-Control directives honored
- [ ] ETag/Last-Modified work
- [ ] Range requests cached

### Cache Invalidation
- [ ] PURGE method works
- [ ] Pattern-based purge
- [ ] TTL expiration works
- [ ] Stale-while-revalidate

### Security
- [ ] Cache poisoning prevented
- [ ] Private responses not cached
- [ ] Credentials not cached
- [ ] Cache key collision safe

### Test Cases
```bash
# Cache hit/miss
curl -v localhost:8080/static/file.js | grep X-Cache
curl -v localhost:8080/static/file.js | grep X-Cache

# Cache purge
curl -X PURGE localhost:8080/static/file.js

# Conditional request
curl -H "If-None-Match: \"abc123\"" localhost:8080/static/file.js
```

---

## Scenario 12: Microservices Discovery

**Source Files:**
- `highper-gateway/src/discovery/`

### Service Discovery
- [ ] Consul integration works
- [ ] etcd integration works
- [ ] Kubernetes integration works
- [ ] Static discovery works
- [ ] Health status reflected

### Dynamic Updates
- [ ] New services discovered
- [ ] Removed services cleaned up
- [ ] Update latency acceptable
- [ ] Watch/poll mode works

### Circuit Breaker
- [ ] Opens after threshold failures
- [ ] Half-open state allows probe
- [ ] Closes after success
- [ ] Per-backend state

### Test Cases
```bash
# Consul registration
curl -X PUT -d '{"ID":"svc1","Name":"api","Address":"127.0.0.1","Port":8081}' localhost:8500/v1/agent/service/register

# Watch for changes
watch -n1 "curl -s localhost:9000/api/backends | jq"

# Circuit breaker test
docker stop backend1
for i in {1..20}; do curl -s -o /dev/null -w "%{http_code}\n" localhost:8080/; done
```

---

## Scenario 13: GraphQL Gateway

**Source Files:**
- `highper-gateway/src/gateway/graphql/`

### Query Handling
- [ ] Query parsing correct
- [ ] Variables substituted
- [ ] Fragments expanded
- [ ] Directives processed

### Security
- [ ] Query depth limited
- [ ] Query complexity limited
- [ ] Introspection controllable
- [ ] Batching limits

### Schema Stitching
- [ ] Multiple schemas merged
- [ ] Type conflicts resolved
- [ ] Field resolution correct

### Test Cases
```bash
# Simple query
curl -X POST -H "Content-Type: application/json" \
  -d '{"query":"{ users { id name } }"}' localhost:8080/graphql

# Introspection
curl -X POST -H "Content-Type: application/json" \
  -d '{"query":"{ __schema { types { name } } }"}' localhost:8080/graphql

# Deep query (should be blocked)
curl -X POST -H "Content-Type: application/json" \
  -d '{"query":"{ a { b { c { d { e { f { g { h { i { j } } } } } } } } } }"}' localhost:8080/graphql
```

---

## Scenario 14: Static Files + PHP-FPM

**Source Files:**
- `highper-gateway/src/webserver/`

### Static File Serving
- [ ] MIME types correct
- [ ] Range requests work
- [ ] Compression (gzip/brotli)
- [ ] Directory index (index.html)
- [ ] Directory listing (if enabled)

### Security
- [ ] Path traversal prevented
- [ ] Symlink following controlled
- [ ] Hidden files protected
- [ ] File permissions checked

### PHP-FPM (FastCGI)
- [ ] SCRIPT_FILENAME correct
- [ ] PATH_INFO correct
- [ ] Query string passed
- [ ] POST body forwarded
- [ ] File uploads work

### Test Cases
```bash
# Static file
curl localhost:8080/static/style.css

# MIME type
curl -I localhost:8080/static/script.js | grep Content-Type

# Path traversal (should fail)
curl localhost:8080/../../../etc/passwd
curl localhost:8080/static/..%2f..%2f..%2fetc/passwd

# PHP execution
curl localhost:8080/info.php
curl -X POST -d "name=test" localhost:8080/api.php
```

---

## Scenario 15: Geographic Load Balancing

**Source Files:**
- `highper-gateway/src/proxy/geographic.rs`

### GeoIP Lookup
- [ ] MaxMind database loads
- [ ] IP2Location database loads
- [ ] Lookup performance acceptable
- [ ] IPv6 supported

### Routing Logic
- [ ] Nearest backend selected
- [ ] Fallback when geo fails
- [ ] X-Forwarded-For respected
- [ ] Override headers work

### Test Cases
```bash
# US East IP
curl -H "X-Forwarded-For: 72.21.206.80" localhost:8080/

# European IP
curl -H "X-Forwarded-For: 82.132.248.70" localhost:8080/

# Unknown IP (fallback)
curl -H "X-Forwarded-For: 0.0.0.0" localhost:8080/
```

---

## Review Sign-Off

### Reviewer Information

| Field | Value |
|-------|-------|
| Reviewer Name | |
| Date | |
| Scenarios Reviewed | |
| Hours Spent | |

### Summary

| Category | Issues Found |
|----------|--------------|
| Critical (P1) | |
| High (P2) | |
| Medium (P3) | |
| Low (P4) | |

### Notes

```
(Add review notes here)
```

---

## Related Documentation

- [BETA_TESTING_GUIDE.md](BETA_TESTING_GUIDE.md) - Testing instructions
- [SECURITY_FEATURES.md](SECURITY_FEATURES.md) - Security documentation
- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture
- [KNOWN_LIMITATIONS.md](../KNOWN_LIMITATIONS.md) - Known issues

---

**Document Version:** 1.0
**Created:** January 2026
**Maintainer:** Highper Gateway Team
