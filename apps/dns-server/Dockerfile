# Multi-stage Dockerfile for DNS server
# Optimized for ARM64 (AWS Graviton) instances

# Stage 1: Build stage
FROM rust:1.91-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy the entire workspace (needed for workspace dependencies)
COPY . .

# Build the application in release mode
# Use --package to build only dns-server from the workspace
RUN cargo build --release --package dns-server

# Stage 2: Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash dnsserver

# Create necessary directories
RUN mkdir -p /etc/dns-server /var/lib/dns-server && \
    chown -R dnsserver:dnsserver /etc/dns-server /var/lib/dns-server

# Copy binary from builder
COPY --from=builder /app/target/release/dns-server /usr/local/bin/dns-server

# Copy example config (can be overridden with volume mount)
# COPY --chown=dnsserver:dnsserver apps/dns-server/local-config.yaml /etc/dns-server/config.yaml

# Switch to non-root user
USER dnsserver

# Expose DNS port (default 53, but configurable)
EXPOSE 53/udp

# Set working directory
WORKDIR /var/lib/dns-server

# Health check (checks if port is listening)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ss -ulnp | grep -q :53 || exit 1

# Run the DNS server
ENTRYPOINT ["/usr/local/bin/dns-server"]

CMD ["--config", "/etc/dns-server/config.yaml"]
