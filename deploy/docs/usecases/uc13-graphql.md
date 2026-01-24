# Use Case 13: GraphQL Gateway

GraphQL-aware gateway with query complexity limiting, persisted queries, and caching.

## Overview

| Property | Value |
|----------|-------|
| Protocol | HTTP/HTTPS |
| Ports | 4000, 443 |
| TLS Required | Optional |
| Privileged | Optional |
| Scaling | Horizontal |

## When to Use

- GraphQL API gateway
- Query complexity protection
- GraphQL caching
- Subscriptions via WebSocket

## Architecture

```
                         ┌──────────────────────┐
    GraphQL:4000 ──────▶ │   Highper Gateway    │
                         │                      │
    Subscriptions ─────▶ │   • Complexity Limit │ ────▶ GraphQL Server
    (WebSocket)          │   • Persisted Queries│
                         │   • Response Caching │
                         └──────────────────────┘
```

## Configuration

### GraphQL Endpoint

```yaml
listeners:
  - name: graphql
    protocol: http
    bind: "0.0.0.0:4000"
    routes:
      - match:
          path: /graphql
        backend: graphql-servers
        graphql:
          enabled: true
      - match:
          path: /subscriptions
        backend: graphql-servers
        websocket:
          enabled: true
```

### Query Complexity

```yaml
graphql:
  complexity:
    enabled: true
    max_depth: 10
    max_complexity: 1000
    max_aliases: 10
```

### Persisted Queries

```yaml
graphql:
  persisted_queries:
    enabled: true
    cache_size: 10000
```

### Response Caching

```yaml
graphql:
  cache:
    enabled: true
    ttl: 60
    private_fields:
      - user
      - me
      - viewer
```

## Rate Limiting

Per-operation rate limits:

```yaml
graphql:
  rate_limiting:
    enabled: true
    default:
      requests_per_second: 100
    by_operation:
      - operation: "heavyQuery"
        requests_per_second: 10
```

## Introspection Control

```yaml
graphql:
  introspection:
    enabled: true        # Enable for development
    cache_ttl: 300       # Cache schema
```

## CORS for GraphQL Playground

```yaml
cors:
  enabled: true
  allow_origins:
    - "*"
  allow_methods:
    - GET
    - POST
    - OPTIONS
  allow_headers:
    - Content-Type
    - Authorization
```

## Subscriptions

WebSocket support for GraphQL subscriptions:

```yaml
websocket:
  enabled: true
  subprotocols:
    - graphql-ws
    - graphql-transport-ws
```

## Monitoring

- `highper_gateway_graphql_queries_total`
- `highper_gateway_graphql_complexity`
- `highper_gateway_graphql_cache_hits_total`

## Related Use Cases

- [UC04: API Gateway](./uc04-api-gateway.md) - REST API gateway
- [UC06: WebSocket](./uc06-websocket.md) - WebSocket only
