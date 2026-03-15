FROM lukemathwalker/cargo-chef:0.1.77-rust-1.94.0-slim-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --package events --recipe-path recipe.json
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --package events

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -m -u 1000 appuser
WORKDIR /app
COPY --from=builder /app/target/release/events ./events
COPY --from=builder /app/events/templates ./templates
COPY --from=builder /app/events/migrations ./migrations
USER appuser
CMD ["./events", "--config", "/app/config/events.yaml"]
