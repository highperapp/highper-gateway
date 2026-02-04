# Scenarios Protocol Summary

Based on scenario filenames and descriptions:

1. **scenario-01-layer4-tcp.proxy** - Layer 4 TCP (Generic TCP)
2. **scenario-02-layer7-http.proxy** - Layer 7 HTTP
3. **scenario-02-layer7-tls-termination.proxy** - Layer 7 HTTPS/TLS Termination
4. **scenario-03-layer7-tls.proxy** - Layer 7 HTTPS/TLS Passthrough
5. **scenario-04-api-gateway.proxy** - HTTP API Gateway
6. **scenario-05-http3-quic.proxy** - HTTP/3 + QUIC
7. **scenario-06-websocket.proxy** - WebSocket
8. **scenario-07-grpc.proxy** - gRPC
9. **scenario-08-database-lb.proxy** - TCP Database Load Balancer (MySQL/PostgreSQL)
10. **scenario-09-waf-mtls.proxy** - HTTP with WAF + mTLS
11. **scenario-10-hybrid-multiprotocol.proxy** - Multiple Protocols
12. **scenario-11-cdn-edge-caching.proxy** - HTTP with Caching
13. **scenario-12-microservices-discovery.proxy** - HTTP Microservices
14. **scenario-13-graphql.proxy** - HTTP GraphQL
15. **scenario-14-static-php-fpm.proxy** - HTTP Static + PHP-FPM
16. **scenario-15-geo-routing.proxy** - HTTP Geographic Routing

## Protocol Categories

### TCP-based (Layer 4):
- Scenario 01: Generic TCP
- Scenario 08: Database (MySQL/PostgreSQL/Redis)

### HTTP-based (Layer 7):
- Scenarios 02, 03, 04, 09-15

### Special Protocols:
- Scenario 05: HTTP/3 + QUIC
- Scenario 06: WebSocket
- Scenario 07: gRPC

## DSL→YAML Converter Requirements

### For TCP Scenarios:
- Need to generate `tcp_routes` section in YAML
- Handle TCP-specific directives

### For HTTP Scenarios:
- Generate `routes` section (already working)
- Handle HTTP-specific middleware

### For Special Protocols:
- HTTP/3: Enable HTTP/3 config section
- WebSocket: Enable websocket middleware
- gRPC: Enable grpc middleware
