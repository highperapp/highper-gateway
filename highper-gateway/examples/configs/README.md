# Configuration Templates

This directory contains production-ready configuration templates for common deployment scenarios.

## Directory Structure

- `yaml/` - YAML configuration files
- `dsl/` - DSL (Caddy-like) configuration files

## Available Templates

### Database Load Balancing
1. **mysql-loadbalancer.yaml** - MySQL database load balancing with connection pooling
2. **postgresql-loadbalancer.yaml** - PostgreSQL load balancing with read/write splitting
3. **redis-loadbalancer.yaml** - Redis cluster load balancing with pipelining

### Web Services
4. **http-api-gateway.yaml** - HTTP/HTTPS REST API gateway with caching and rate limiting
5. **grpc-gateway.yaml** - gRPC service gateway with HTTP/2 multiplexing
6. **graphql-gateway.yaml** - GraphQL API gateway with query complexity limits
7. **websocket-gateway.yaml** - WebSocket gateway for real-time applications

### Edge & CDN
8. **static-files-cdn.yaml** - Static file serving with aggressive caching and HTTP/3
9. **http3-edge-server.yaml** - HTTP/3 edge server with QUIC and 0-RTT

### Application Gateways
10. **php-fpm-gateway.yaml** - PHP-FPM application gateway with FastCGI support

### Advanced Scenarios
11. **multi-protocol-gateway.yaml** - Multi-protocol gateway (HTTP/gRPC/WebSocket)
12. **service-mesh-sidecar.yaml** - Service mesh sidecar with mTLS and observability
13. **api-rate-limiting.yaml** - API gateway with advanced rate limiting
14. **waf-protected-gateway.yaml** - WAF-protected gateway with security rules
15. **mtls-gateway.yaml** - Mutual TLS gateway with client certificate authentication

## Usage

### Using YAML Configuration

```bash
# Start with a specific template
highper-gateway --config examples/configs/yaml/mysql-loadbalancer.yaml

# Validate a configuration
highper-gateway --config examples/configs/yaml/http-api-gateway.yaml --validate
```

### Using DSL Configuration

```bash
# Start with DSL configuration
highper-gateway --config examples/configs/dsl/mysql-loadbalancer.hcl

# Convert DSL to YAML
highper-gateway --convert examples/configs/dsl/http-api-gateway.hcl > config.yaml
```

## Customization

All templates use environment variables for sensitive data and deployment-specific values:

- `${BACKEND_HOST}` - Backend server hostname/IP
- `${BACKEND_PORT}` - Backend server port
- `${TLS_CERT_PATH}` - Path to TLS certificate
- `${TLS_KEY_PATH}` - Path to TLS private key
- `${ADMIN_API_KEY}` - Admin API authentication key

Set these in your environment before starting:

```bash
export BACKEND_HOST=192.168.1.100
export BACKEND_PORT=3306
export ADMIN_API_KEY=your-secret-key
highper-gateway --config examples/configs/yaml/mysql-loadbalancer.yaml
```

## Protocol-Specific Defaults

Each template leverages the built-in protocol defaults from `ProtocolDefaults`:

- **MySQL/PostgreSQL**: Long-lived connections, connection pooling, pre-warming
- **Redis**: High-throughput, short-lived connections, pipeline support
- **HTTP API**: Caching, rate limiting, security headers, compression
- **gRPC**: HTTP/2 multiplexing, streaming, health checking
- **WebSocket**: Long-lived bidirectional connections, ping/pong keepalive
- **GraphQL**: Query complexity limits, batching, subscriptions
- **Static Files**: Aggressive caching, HTTP/3, compression
- **PHP-FPM**: FastCGI, file upload handling, script timeouts
- **HTTP/3**: QUIC protocol, 0-RTT, connection migration

## Best Practices

1. **Start with a template** - Choose the template closest to your use case
2. **Customize for production** - Update hostnames, ports, and credentials
3. **Enable observability** - Keep metrics and logging enabled in production
4. **Use environment variables** - Never commit secrets to version control
5. **Test configuration** - Always validate before deploying
6. **Monitor performance** - Use the built-in metrics endpoint at `:9090/metrics`
7. **Enable health checks** - Configure upstream health checking for high availability

## Performance Tuning

Templates include production-tested default values, but you may need to tune for your specific workload:

- **max_connections** - Increase for high-traffic scenarios
- **connection_pool.max_idle_per_host** - Tune based on backend capacity
- **request_timeout** - Adjust based on backend response times
- **cache.default_ttl** - Tune based on content freshness requirements
- **rate_limit.capacity** - Set based on API quotas and fair usage

## Security Considerations

All templates include security best practices:

- TLS 1.2+ minimum version
- Modern cipher suites
- Security headers (HSTS, CSP, etc.)
- Rate limiting to prevent abuse
- Admin API authentication
- Optional WAF integration
- Optional mTLS for zero-trust networking

## Support

For questions or issues:
- Documentation: https://docs.highper-gateway.io
- GitHub: https://github.com/highper/highper-gateway
- Issues: https://github.com/highper/highper-gateway/issues
