# Multi-stage Dockerfile for DNS server
# Optimized for ARM64 (AWS Graviton) instances

FROM lukemathwalker/cargo-chef:0.1.77-rust-1.94.0-slim-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo chef cook --release --package dns-server --recipe-path recipe.json
COPY . .
RUN cargo build --release --package dns-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -m -u 1000 appuser
RUN mkdir -p /etc/dns-server /var/lib/dns-server && \
    chown -R appuser:appuser /etc/dns-server /var/lib/dns-server
COPY --from=builder /app/target/release/dns-server /usr/local/bin/dns-server
USER appuser
EXPOSE 53/udp
WORKDIR /var/lib/dns-server
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ss -ulnp | grep -q :53 || exit 1
ENTRYPOINT ["/usr/local/bin/dns-server"]
CMD ["--config", "/etc/dns-server/config.yaml"]
