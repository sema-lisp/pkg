FROM rust:1.88-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY templates/ templates/
RUN cargo build --release

FROM debian:bookworm-slim
# curl backs the container HEALTHCHECK / compose readiness probe against /readyz.
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/sema-pkg /usr/local/bin/
COPY templates/ templates/
COPY static/ static/
EXPOSE 3000
VOLUME ["/app/data"]
HEALTHCHECK --interval=15s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -fsS http://localhost:3000/readyz || exit 1
CMD ["sema-pkg"]
