# Highper Gateway Dockerfile for Load Testing
# Multi-stage build for minimal image size

# Build stage
FROM rust:1.83-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev

WORKDIR /build

# Copy workspace Cargo.toml
COPY Cargo.toml Cargo.lock ./

# Copy package
COPY highper-gateway ./highper-gateway

# Build release binary
WORKDIR /build/highper-gateway
RUN cargo build --release

# Runtime stage
FROM alpine:latest

# Install runtime dependencies
RUN apk add --no-cache libgcc openssl ca-certificates curl wget

# Copy binary from builder
COPY --from=builder /build/target/release/highper-gateway /usr/local/bin/highper-gateway

# Create necessary directories
RUN mkdir -p /etc/highper-gateway /var/log/highper-gateway /etc/highper-gateway/certs

# Create non-root user
RUN addgroup -g 1000 gateway && \
    adduser -D -u 1000 -G gateway gateway && \
    chown -R gateway:gateway /var/log/highper-gateway

# Expose ports
EXPOSE 8080 8443 8081 9000 9090 50051
EXPOSE 8443/udp

USER gateway

# Health check
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 \
    CMD wget -q -O - http://localhost:9090/health || exit 1

ENTRYPOINT ["/usr/local/bin/highper-gateway"]
CMD ["--config", "/etc/highper-gateway/config.toml"]
