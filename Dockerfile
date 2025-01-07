
FROM rust:latest AS builder
WORKDIR /app
COPY . .
RUN cargo build --bin api --no-default-features --features api --release

FROM rust:latest
COPY --from=builder /app/target/release/api /app/api
COPY --from=builder /app/config-api-example.toml /app/config.toml
WORKDIR /app
CMD ["api", "--config", "config.toml"]

