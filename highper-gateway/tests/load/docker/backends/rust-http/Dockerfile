# Build stage
FROM rust:1.83-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /build

# Copy manifest
COPY Cargo.toml ./

# Copy source
COPY src ./src

# Build release binary
RUN cargo build --release

# Runtime stage
FROM alpine:latest

# Copy binary from builder
COPY --from=builder /build/target/release/load-test-backend /usr/local/bin/backend

# Create non-root user
RUN addgroup -g 1000 backend && \
    adduser -D -u 1000 -G backend backend

USER backend

# Expose port (configurable via ENV)
EXPOSE 8000

# Run backend
ENTRYPOINT ["/usr/local/bin/backend"]
