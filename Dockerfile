FROM rust:1.88-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY templates/ templates/
# include_str!'d at build time (the Sema README-highlighting grammar).
COPY syntaxes/ syntaxes/
RUN cargo build --release

FROM debian:bookworm-slim
# curl backs the container HEALTHCHECK / compose readiness probe against /readyz.
# (User admin is `sema-pkg admin …`, not raw DB edits — no sqlite3 needed.)
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/sema-pkg /usr/local/bin/
# Litestream, for optional in-container continuous SQLite backup to S3/R2/Tigris
# (enabled with LITESTREAM_REPLICATE=1 — see entrypoint.sh). Pinned + copied from
# the official image so the runtime image stays slim and reproducible.
COPY --from=litestream/litestream:0.5.14 /usr/local/bin/litestream /usr/local/bin/litestream
COPY litestream.yml /etc/litestream.yml
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
COPY templates/ templates/
COPY static/ static/
EXPOSE 3000
VOLUME ["/app/data"]
HEALTHCHECK --interval=15s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -fsS "http://localhost:${PORT:-3000}/readyz" || exit 1
# entrypoint.sh execs `sema-pkg` directly by default; with LITESTREAM_REPLICATE=1
# it wraps the app in `litestream replicate -exec` for built-in backup.
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
