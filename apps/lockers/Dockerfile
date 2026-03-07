FROM lukemathwalker/cargo-chef:0.1.77-rust-1.94.0-slim-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --package lockers --recipe-path recipe.json
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --package lockers

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -m -u 1000 appuser
WORKDIR /app
COPY --from=builder /app/target/release/lockers ./lockers
COPY --from=builder /app/lockers/templates ./templates
COPY --from=builder /app/lockers/assets ./assets
COPY --from=builder /app/lockers/migrations ./migrations
USER appuser
CMD ["./lockers", "--config", "/app/config/lockers.yaml"]
