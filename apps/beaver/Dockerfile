FROM lukemathwalker/cargo-chef:0.1.78-rust-1.100.0-slim-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
# The bundled DuckDB is compiled from C++ source
RUN apt-get update \
    && apt-get install -y g++ \
    && rm -rf /var/lib/apt/lists/*
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --package beaver --recipe-path recipe.json
COPY . .
RUN cargo build --release --package beaver

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y ca-certificates libstdc++6 \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -m -u 1000 appuser
WORKDIR /app
COPY --from=builder /app/target/release/beaver ./beaver
USER appuser
CMD ["./beaver", "--config", "/app/config/beaver.yaml"]
