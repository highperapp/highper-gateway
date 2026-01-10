# Multi-stage build for Rust Reverse Proxy
# Stage 1: Build the application
FROM rust:latest AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY highper-gateway/Cargo.toml ./highper-gateway/

# Copy source code
COPY highper-gateway/src ./highper-gateway/src
COPY highper-gateway/benches ./highper-gateway/benches
COPY highper-gateway/examples ./highper-gateway/examples

# Build for release
RUN cd highper-gateway && cargo build --release

# Stage 2: Create minimal runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 1000 -m highper-gateway

# Copy binary from builder
COPY --from=builder /app/highper-gateway/target/release/highper-gateway /usr/local/bin/

# Create directories
RUN mkdir -p /etc/highper-gateway /var/log/highper-gateway /var/lib/highper-gateway \
    && chown -R highper-gateway:highper-gateway /var/log/highper-gateway /var/lib/highper-gateway

# Switch to non-root user
USER highper-gateway

# Expose ports
EXPOSE 8080 8443 9090

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/highper-gateway", "health"]

# Set working directory
WORKDIR /etc/highper-gateway

# Run the proxy
ENTRYPOINT ["/usr/local/bin/highper-gateway"]
CMD ["start", "--config", "/etc/highper-gateway/config.toml"]
