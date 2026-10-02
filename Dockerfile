# Ecclesia on Fly: workspace build (root Cargo.toml has no [package]).
# cargo-chef caches dependency layers when only app source changes.
FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/domain/Cargo.toml crates/domain/Cargo.toml
COPY crates/sdk/Cargo.toml crates/sdk/Cargo.toml
COPY crates/app/Cargo.toml crates/app/Cargo.toml
RUN mkdir -p crates/domain/src crates/sdk/src crates/app/src/bin \
    && printf 'pub fn _chef_stub() {}\n' > crates/domain/src/lib.rs \
    && printf 'pub fn _chef_stub() {}\n' > crates/sdk/src/lib.rs \
    && printf 'pub fn _chef_stub() {}\n' > crates/app/src/lib.rs \
    && printf 'fn main() {}\n' > crates/app/src/main.rs \
    && printf 'fn main() {}\n' > crates/app/src/bin/worker.rs
RUN cargo chef cook --release --recipe-path recipe.json -p ecclesia
COPY . .
RUN cargo build --release -p ecclesia --bin ecclesia --bin ecclesia-worker

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
# CARGO_MANIFEST_DIR is baked in at compile time as /app/crates/app.
COPY --from=builder /app/target/release/ecclesia /ecclesia
COPY --from=builder /app/target/release/ecclesia-worker /ecclesia-worker
COPY --from=builder /app/crates/app/static /app/crates/app/static
ENV PORT=8080
EXPOSE 8080
