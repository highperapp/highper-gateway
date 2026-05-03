# Highper Gateway - Multi-stage Dockerfile
# Optimized for production with minimal attack surface

# Build stage
# - Cargo.lock is version 4 (needs Rust 1.78+).
# - Several deps (async-graphql-value v7.0.17 etc.) require edition 2024,
#   stabilized in Rust 1.85.
# Pinning 1.86 for a stable, reproducible base.
FROM rust:1.86-bookworm AS builder

# Install build dependencies:
# - liburing-dev: required by the io-uring crate (Linux-only async I/O).
# - pkg-config: link-step tooling for sys crates.
# - cmake, nasm, perl: required by quiche v0.24+ to build its bundled
#   boringssl. cmake drives the build; nasm assembles x86 crypto;
#   perl runs the boringssl code-generation scripts.
# - libssl-dev, ca-certificates: belt-and-suspenders for any sys crate
#   that prefers system OpenSSL over a vendored copy.
RUN apt-get update && apt-get install -y \
    liburing-dev \
    pkg-config \
    cmake \
    nasm \
    perl \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy only Cargo files first to cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY highper-gateway/Cargo.toml highper-gateway/
# build.rs sets RUSTC_VERSION/TARGET/PROFILE/BUILD_DATE env vars that
# main.rs reads via env!() in the version_command. Copying it before the
# dummy build keeps the dep-cache layer valid and ensures the script runs.
COPY highper-gateway/build.rs highper-gateway/build.rs

# xtask is a workspace member (CI lint binary). Tiny crate; copy whole so
# cargo can resolve the workspace during the dummy-build dependency cache.
# Not built here — the --package flag below restricts to highper-gateway.
COPY xtask xtask

# Create dummy sources to satisfy manifest parsing during the dependency
# cache build. Cargo refuses to parse the manifest when [[bench]] target
# paths don't exist, even with --package highper-gateway.
RUN mkdir -p highper-gateway/src highper-gateway/benches && \
    echo "fn main() {}" > highper-gateway/src/main.rs && \
    echo "fn main() {}" > highper-gateway/benches/proxy_bench.rs && \
    echo "fn main() {}" > highper-gateway/benches/tcp_bench.rs && \
    echo "fn main() {}" > highper-gateway/benches/optimization_bench.rs && \
    echo "fn main() {}" > highper-gateway/benches/buffer_pool_steady_state.rs && \
    cargo build --release --package highper-gateway && \
    rm -rf highper-gateway/src
# Note: benches/ dummies retained so the next cargo build can parse the
# manifest. `--package highper-gateway --release` does not compile benches
# (only `cargo bench` would), so dummy fn main() {} files are sufficient.

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
