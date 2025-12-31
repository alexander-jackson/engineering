FROM lukemathwalker/cargo-chef:0.1.72-rust-1.89.0 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --package tag-updater --recipe-path recipe.json

COPY . .
RUN cargo build --release --package tag-updater

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm AS runtime

RUN apt-get update && apt-get install -y pkg-config libssl-dev ssh ca-certificates && ssh-keyscan -t rsa github.com >> ~/.ssh/known_hosts

WORKDIR /app
COPY --from=builder /app/target/release/tag-updater ./tag-updater

CMD ["./tag-updater", "--config", "/app/config/tag-updater.yaml"]
