# Highper Gateway - Multi-stage Dockerfile
# Optimized for production with minimal attack surface

# Build stage
FROM rust:1.75-bookworm AS builder

# Install build dependencies including io_uring
RUN apt-get update && apt-get install -y \
    liburing-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy only Cargo files first to cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY highper-gateway/Cargo.toml highper-gateway/

# xtask is a workspace member (CI lint binary). Tiny crate; copy whole so
# cargo can resolve the workspace during the dummy-build dependency cache.
# Not built here — the --package flag below restricts to highper-gateway.
COPY xtask xtask

# Create dummy source to build dependencies (cached when Cargo files unchanged)
RUN mkdir -p highper-gateway/src && \
    echo "fn main() {}" > highper-gateway/src/main.rs && \
    cargo build --release --package highper-gateway && \
    rm -rf highper-gateway/src

# Copy actual source code
COPY highper-gateway/src highper-gateway/src

# Build the actual application
RUN touch highper-gateway/src/main.rs && \
    cargo build --release --package highper-gateway

# Runtime stage - Distroless for minimal attack surface
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

# Copy binary
COPY --from=builder /app/target/release/highper-gateway /usr/local/bin/highper-gateway

# Copy liburing (required at runtime)
COPY --from=builder /usr/lib/x86_64-linux-gnu/liburing.so* /usr/lib/x86_64-linux-gnu/

# Create config directory
WORKDIR /etc/highper-gateway

# Default configuration
COPY deploy/docker/default-config.yaml /etc/highper-gateway/config.yaml

# Expose common ports
EXPOSE 80 443 8080 8081 9090

# Health check
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=3 \
    CMD ["/usr/local/bin/highper-gateway", "--health-check"]

# Run as non-root
USER nonroot:nonroot

ENTRYPOINT ["/usr/local/bin/highper-gateway"]
CMD ["--config", "/etc/highper-gateway/config.yaml"]
