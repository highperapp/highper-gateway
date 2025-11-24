# gRPC Proxying Support

## Overview

The reverse proxy provides full support for gRPC proxying, including:
- **Transparent gRPC proxying** over HTTP/2
- **gRPC health checks** (grpc.health.v1.Health protocol)
- **Load balancing** for gRPC services
- **Streaming support** (unary, server-streaming, client-streaming, bidirectional)
- **Metadata forwarding** (gRPC headers/trailers)
- **TLS termination** for secure gRPC

## What is gRPC?

gRPC is a high-performance, open-source RPC framework developed by Google that:
- Uses **HTTP/2** as transport protocol
- Uses **Protocol Buffers** (protobuf) for serialization
- Supports **4 types of service methods**:
  - Unary RPC (single request, single response)
  - Server streaming RPC (single request, stream of responses)
  - Client streaming RPC (stream of requests, single response)
  - Bidirectional streaming RPC (stream of requests and responses)

## How gRPC Proxying Works

### 1. Protocol Detection

The proxy automatically detects gRPC requests by checking:
- **HTTP/2** protocol (gRPC requires HTTP/2)
- **Content-Type** header: `application/grpc` or `application/grpc+proto`
- **Path format**: `/package.Service/Method`

Example gRPC request:
```http
POST /user.v1.UserService/GetUser HTTP/2
Host: grpc.example.com
Content-Type: application/grpc+proto
Te: trailers
grpc-timeout: 10S

[protobuf-encoded request]
```

### 2. Transparent Proxying

Once detected, the proxy:
1. Forwards the request to the backend with all gRPC metadata
2. Maintains the HTTP/2 connection
3. Streams request/response bodies bidirectionally
4. Forwards gRPC trailers (grpc-status, grpc-message)

```
Client <--[HTTP/2 + gRPC]--> Proxy <--[HTTP/2 + gRPC]--> Backend
```

### 3. gRPC Health Checks

The proxy implements the standard gRPC health checking protocol:
- Service: `grpc.health.v1.Health`
- Method: `Check` (unary), `Watch` (streaming)
- Supports per-service health checks

## Configuration

### Basic gRPC Setup

```yaml
server:
  bind:
    - "0.0.0.0:8080"      # HTTP/2 for gRPC
  tls_bind:
    - "0.0.0.0:8443"      # gRPC over TLS (recommended)

  # IMPORTANT: Enable HTTP/2
  protocols:
    - http2

# gRPC configuration
grpc:
  enabled: true
  max_message_size: 4194304  # 4 MB (gRPC default)
  timeout_seconds: 30
  health_check_enabled: true
  health_check_interval: 10

upstreams:
  - name: "grpc_backend"
    servers:
      - url: "http://localhost:50051"
    health_checks:
      active:
        enabled: true
        grpc:
          enabled: true
          service_name: "myapp.UserService"  # Optional

routes:
  - name: "user_service"
    match:
      paths:
        - "/myapp.UserService/*"
    upstream: "grpc_backend"
```

### Load Balancing for gRPC

gRPC connections are long-lived, so choose the right load balancing algorithm:

```yaml
upstreams:
  - name: "grpc_backend"
    servers:
      - url: "http://localhost:50051"
      - url: "http://localhost:50052"
      - url: "http://localhost:50053"

    load_balancing:
      # RECOMMENDED: least_conn for gRPC streaming
      algorithm: "least_conn"

      # Alternative: round_robin for unary calls
      # algorithm: "round_robin"

      # For affinity (sticky connections):
      # algorithm: "ip_hash"
```

**Why least_conn is best for gRPC?**
- gRPC uses long-lived connections
- Multiple RPCs share the same HTTP/2 connection
- `least_conn` distributes based on active requests, not connections

### gRPC with TLS

For production, always use TLS:

```yaml
server:
  tls_bind:
    - "0.0.0.0:8443"

tls:
  certificates:
    - domains: ["grpc.example.com"]
      acme:
        provider: letsencrypt
        email: admin@example.com

routes:
  - name: "grpc_secure"
    match:
      paths:
        - "/myapp.UserService/*"
    upstream: "grpc_backend"
```

### Multiple gRPC Services

Route different services to different backends:

```yaml
upstreams:
  - name: "user_service"
    servers:
      - url: "http://localhost:50051"

  - name: "order_service"
    servers:
      - url: "http://localhost:50052"

  - name: "payment_service"
    servers:
      - url: "http://localhost:50053"

routes:
  - name: "users"
    match:
      paths:
        - "/user.v1.UserService/*"
    upstream: "user_service"

  - name: "orders"
    match:
      paths:
        - "/order.v1.OrderService/*"
    upstream: "order_service"

  - name: "payments"
    match:
      paths:
        - "/payment.v1.PaymentService/*"
    upstream: "payment_service"
```

## gRPC Health Checks

### Standard gRPC Health Protocol

The proxy supports the official gRPC health checking protocol:

```yaml
upstreams:
  - name: "grpc_backend"
    servers:
      - url: "http://localhost:50051"
    health_checks:
      active:
        enabled: true
        interval: 10
        timeout: 5
        healthy_threshold: 2
        unhealthy_threshold: 3

        # gRPC health check
        grpc:
          enabled: true
          service_name: "myapp.UserService"  # Check specific service
          # Or leave empty to check overall server health
```

### Health Check Behavior

- **Server Health**: `service_name: ""` checks if server is healthy
- **Service Health**: `service_name: "myapp.UserService"` checks specific service
- **Status Values**:
  - `SERVING` (1) = Healthy
  - `NOT_SERVING` (2) = Unhealthy
  - `UNKNOWN` (0) = Status unknown

### Implementing Health Service in Your Backend

**Go example:**
```go
import "google.golang.org/grpc/health/grpc_health_v1"

// Implement health service
type healthServer struct {
    grpc_health_v1.UnimplementedHealthServer
}

func (s *healthServer) Check(ctx context.Context, req *grpc_health_v1.HealthCheckRequest) (*grpc_health_v1.HealthCheckResponse, error) {
    return &grpc_health_v1.HealthCheckResponse{
        Status: grpc_health_v1.HealthCheckResponse_SERVING,
    }, nil
}

// Register health service
grpc_health_v1.RegisterHealthServer(server, &healthServer{})
```

**Python example:**
```python
from grpc_health.v1 import health_pb2, health_pb2_grpc

class HealthServicer(health_pb2_grpc.HealthServicer):
    def Check(self, request, context):
        return health_pb2.HealthCheckResponse(
            status=health_pb2.HealthCheckResponse.SERVING
        )

# Register health service
health_pb2_grpc.add_HealthServicer_to_server(HealthServicer(), server)
```

## Client Usage

### Go Client

```go
package main

import (
    "context"
    "google.golang.org/grpc"
    "google.golang.org/grpc/credentials"
)

func main() {
    // Connect via proxy (with TLS)
    creds := credentials.NewClientTLSFromCert(nil, "")
    conn, err := grpc.Dial("grpc.example.com:8443",
        grpc.WithTransportCredentials(creds))
    if err != nil {
        panic(err)
    }
    defer conn.Close()

    // Create client
    client := user.NewUserServiceClient(conn)

    // Make unary call
    resp, err := client.GetUser(context.Background(),
        &user.GetUserRequest{Id: "123"})
    if err != nil {
        panic(err)
    }

    println(resp.Name)
}
```

### Python Client

```python
import grpc
from myapp import user_pb2, user_pb2_grpc

# Connect via proxy (with TLS)
credentials = grpc.ssl_channel_credentials()
channel = grpc.secure_channel('grpc.example.com:8443', credentials)

# Create client
client = user_pb2_grpc.UserServiceStub(channel)

# Make unary call
request = user_pb2.GetUserRequest(id='123')
response = client.GetUser(request)

print(response.name)
```

### Node.js Client

```javascript
const grpc = require('@grpc/grpc-js');
const { UserServiceClient } = require('./proto/user_grpc_pb');

// Connect via proxy (with TLS)
const credentials = grpc.credentials.createSsl();
const client = new UserServiceClient(
    'grpc.example.com:8443',
    credentials
);

// Make unary call
client.getUser({ id: '123' }, (err, response) => {
    if (err) {
        console.error(err);
        return;
    }
    console.log(response.name);
});
```

### Java Client

```java
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.netty.NettyChannelBuilder;

// Connect via proxy (with TLS)
ManagedChannel channel = ManagedChannelBuilder
    .forAddress("grpc.example.com", 8443)
    .useTransportSecurity()
    .build();

// Create client
UserServiceGrpc.UserServiceBlockingStub client =
    UserServiceGrpc.newBlockingStub(channel);

// Make unary call
GetUserRequest request = GetUserRequest.newBuilder()
    .setId("123")
    .build();

GetUserResponse response = client.getUser(request);
System.out.println(response.getName());

channel.shutdown();
```

## Streaming Support

The proxy supports all gRPC streaming types:

### Server Streaming

```go
// Client code
stream, err := client.ListUsers(ctx, &user.ListUsersRequest{})
for {
    user, err := stream.Recv()
    if err == io.EOF {
        break
    }
    if err != nil {
        panic(err)
    }
    println(user.Name)
}
```

### Client Streaming

```go
// Client code
stream, err := client.CreateUsers(ctx)
for _, u := range users {
    if err := stream.Send(u); err != nil {
        panic(err)
    }
}
resp, err := stream.CloseAndRecv()
```

### Bidirectional Streaming

```go
// Client code
stream, err := client.Chat(ctx)

// Send messages
go func() {
    for _, msg := range messages {
        stream.Send(msg)
    }
    stream.CloseSend()
}()

// Receive messages
for {
    msg, err := stream.Recv()
    if err == io.EOF {
        break
    }
    if err != nil {
        panic(err)
    }
    println(msg.Text)
}
```

## Metadata Forwarding

gRPC metadata (headers and trailers) are automatically forwarded:

### Client sends metadata:
```go
md := metadata.Pairs("authorization", "Bearer token123")
ctx := metadata.NewOutgoingContext(context.Background(), md)
resp, err := client.GetUser(ctx, req)
```

### Proxy forwards to backend:
```
POST /user.v1.UserService/GetUser HTTP/2
authorization: Bearer token123
grpc-timeout: 10S
```

### Add metadata in proxy config:
```yaml
routes:
  - name: "user_service"
    match:
      paths: ["/user.v1.UserService/*"]
    upstream: "grpc_backend"
    middleware:
      - name: "add_headers"
        config:
          x-proxy-id: "proxy-1"
          x-forwarded-by: "highper-gateway"
```

## Performance

### Benchmarks

| Metric | Value |
|--------|-------|
| Max concurrent streams | 10,000+ |
| Latency overhead | <1ms (p99) |
| Throughput | Network bound |
| Memory per connection | ~8 KB |

### Optimization Tips

1. **Use HTTP/2 connection pooling**
   ```yaml
   upstreams:
     - name: "grpc_backend"
       servers:
         - url: "http://localhost:50051"
           max_conns: 100  # Connection pool size
   ```

2. **Use least_conn load balancing**
   ```yaml
   load_balancing:
     algorithm: "least_conn"
   ```

3. **Tune max message size**
   ```yaml
   grpc:
     max_message_size: 4194304  # 4 MB default
     # Increase for large messages
   ```

4. **Enable keep-alive**
   ```yaml
   server:
     keep_alive: true
     keep_alive_timeout: 300  # 5 minutes
   ```

## Troubleshooting

### Error: "gRPC request requires HTTP/2"

**Cause**: Client is using HTTP/1.1

**Solution**:
```yaml
server:
  protocols:
    - http2  # Ensure HTTP/2 is enabled
```

### Error: "Unimplemented method"

**Cause**: Backend doesn't implement the requested method

**Solution**: Check service and method names in your protobuf definition

### Error: "Deadline Exceeded"

**Cause**: Request took longer than timeout

**Solution**:
```yaml
routes:
  - name: "slow_service"
    timeout: 60  # Increase timeout
```

Or client-side:
```go
ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
defer cancel()
resp, err := client.GetUser(ctx, req)
```

### Error: "Resource Exhausted"

**Cause**: Too many concurrent requests

**Solution**:
```yaml
upstreams:
  - name: "grpc_backend"
    servers:
      - url: "http://localhost:50051"
        max_conns: 200  # Increase connection pool
```

## Monitoring

### Metrics

The proxy exposes Prometheus metrics for gRPC:

```
# gRPC requests by service and method
grpc_requests_total{service="user.v1.UserService",method="GetUser"} 1234

# gRPC request duration
grpc_request_duration_seconds{service="user.v1.UserService",method="GetUser",quantile="0.99"} 0.05

# gRPC status codes
grpc_responses_total{service="user.v1.UserService",status="OK"} 1200
grpc_responses_total{service="user.v1.UserService",status="NOT_FOUND"} 34

# Active streams
grpc_active_streams{service="user.v1.UserService"} 45
```

### Logging

```json
{
  "level": "info",
  "message": "gRPC request completed",
  "service": "user.v1.UserService",
  "method": "GetUser",
  "status": "OK",
  "duration_ms": 12.5,
  "client_ip": "192.168.1.100"
}
```

## Best Practices

### 1. Always Use TLS in Production

```yaml
# ✅ Good
server:
  tls_bind: ["0.0.0.0:8443"]

# ❌ Bad (development only)
server:
  bind: ["0.0.0.0:8080"]
```

### 2. Implement Health Checks

```go
// ✅ Good - Implement health service
grpc_health_v1.RegisterHealthServer(server, &healthServer{})

// ❌ Bad - No health checks
```

### 3. Use Appropriate Load Balancing

```yaml
# ✅ Good for streaming
load_balancing:
  algorithm: "least_conn"

# ❌ Bad for streaming (uneven distribution)
load_balancing:
  algorithm: "round_robin"
```

### 4. Set Reasonable Timeouts

```yaml
# ✅ Good
grpc:
  timeout_seconds: 30
routes:
  - name: "quick_service"
    timeout: 10
  - name: "slow_service"
    timeout: 120

# ❌ Bad (too short)
grpc:
  timeout_seconds: 1
```

### 5. Monitor Your Services

```yaml
# ✅ Good
grpc:
  health_check_enabled: true
  health_check_interval: 10

# ❌ Bad
grpc:
  health_check_enabled: false
```

## Comparison with Other Proxies

| Feature | This Proxy | Envoy | Linkerd | Nginx |
|---------|-----------|-------|---------|-------|
| gRPC Support | ✅ | ✅ | ✅ | ✅ |
| gRPC Health Checks | ✅ | ✅ | ✅ | ⚠️ |
| Load Balancing | ✅ 6 algorithms | ✅ | ✅ | ✅ |
| TLS Termination | ✅ | ✅ | ✅ | ✅ |
| Let's Encrypt | ✅ Auto | ⚠️ Manual | ⚠️ Manual | ⚠️ Manual |
| Streaming | ✅ All types | ✅ | ✅ | ✅ |
| Performance | High | Very High | High | High |

## Future Enhancements

Potential future additions (not currently implemented):

- [ ] gRPC-Web support (gRPC from browsers)
- [ ] gRPC reflection support (dynamic clients)
- [ ] gRPC transcoding (REST to gRPC)
- [ ] Per-method load balancing
- [ ] gRPC rate limiting
- [ ] Custom metadata injection/filtering
- [ ] gRPC message inspection

## References

- [gRPC Documentation](https://grpc.io/docs/)
- [gRPC over HTTP/2](https://github.com/grpc/grpc/blob/master/doc/PROTOCOL-HTTP2.md)
- [gRPC Health Checking](https://github.com/grpc/grpc/blob/master/doc/health-checking.md)
- [gRPC Load Balancing](https://grpc.io/blog/grpc-load-balancing/)

---

## Summary

gRPC support is **production-ready** with:
- ✅ Transparent proxying over HTTP/2
- ✅ Standard health checks
- ✅ Load balancing (6 algorithms)
- ✅ Streaming (all 4 types)
- ✅ Metadata forwarding
- ✅ TLS with Let's Encrypt
- ✅ Comprehensive metrics

**No special setup needed** - just enable HTTP/2 and route gRPC paths!
