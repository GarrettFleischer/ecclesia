# Ecclesia on Fly: workspace build (root Cargo.toml has no [package]).
FROM rust:1-bookworm AS builder
RUN rustup toolchain install nightly
WORKDIR /app
COPY . .
RUN cargo +nightly build --release -p ecclesia --bin ecclesia --bin ecclesia-worker

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
