FROM rust:1-bookworm

# Install musl target and cargo-deb
RUN rustup target add aarch64-unknown-linux-musl && \
    apt-get update && \
    apt-get install -y musl-tools && \
    cargo install cargo-deb && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
