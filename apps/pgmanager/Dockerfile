FROM lukemathwalker/cargo-chef:0.1.72-rust-1.89.0 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --package pgmanager --recipe-path recipe.json

COPY . .
RUN cargo build --release --package pgmanager

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y postgresql-client-15

WORKDIR /app
COPY --from=builder /app/target/release/pgmanager ./pgmanager

CMD ["./pgmanager", "--config", "/app/config/pgmanager.yaml"]

