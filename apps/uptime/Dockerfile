# Use a Rust base image with Cargo installed
FROM rust:1.89.0 AS builder

# Set the working directory inside the container
WORKDIR /usr/src/app

# Copy the Cargo.toml and Cargo.lock files
COPY . .

# Build your application
ENV SQLX_OFFLINE=true
RUN cargo build --bin uptime --release

# Start a new stage to create a smaller image without unnecessary build dependencies
FROM debian:trixie-slim

# Set the working directory
WORKDIR /usr/src/app

# Copy the built binary from the previous stage, as well as the runtime files
COPY --from=builder /usr/src/app/uptime/templates ./templates
COPY --from=builder /usr/src/app/uptime/assets ./assets
COPY --from=builder /usr/src/app/target/release/uptime ./

# Command to run the application
CMD ["./uptime", "--config", "/app/config/uptime.yaml"]
