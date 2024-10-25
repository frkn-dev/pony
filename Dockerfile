
FROM rust:latest AS builder
WORKDIR /app
COPY . .
RUN cargo build --release 

FROM rust:latest
COPY --from=builder /app/target/release/pony /app/pony
COPY --from=builder /app/config-example.toml /app/config.toml
WORKDIR /app
CMD ["pony", "--config", "config.toml"]

