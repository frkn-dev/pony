FROM rust:latest AS builder

RUN rustup install stable-x86_64-unknown-linux-musl

RUN rustup target add x86_64-unknown-linux-musl
RUN apt -y update
RUN apt install -y musl-tools musl-dev
RUN apt-get install -y build-essential
RUN apt install -y gcc-x86-64-linux-gnu
RUN apt-get install -y pkg-config libssl-dev

ENV RUSTFLAGS='-C linker=x86_64-linux-gnu-gcc'
ENV CC='gcc'
ENV CC_x86_64_unknown_linux_musl=x86_64-linux-gnu-gcc

ENV OPENSSL_DIR=/usr/
ENV OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
ENV OPENSSL_INCLUDE_DIR=/usr/include/openssl

WORKDIR /code
COPY . .

RUN ls /usr/include/openssl && echo "OpenSSL headers found"
RUN cargo build --target x86_64-unknown-linux-musl --release
RUN ls /code/target/x86_64-unknown-linux-musl/release
