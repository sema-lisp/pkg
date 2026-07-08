# Jakefile — sema-pkg (self-hostable Sema package registry). The sole task runner
# for this repo (no Makefile). `@rooted` so the sema-lisp/workspace meta-repo can
# `@import "pkg/Jakefile" as pkg` and run `pkg.dev` / `pkg.test` from the root.
@rooted

# ── Build / quality ──────────────────────────────────────────

@group build
@desc "Build the registry (debug)"
task build:
    cargo build

@group build
@desc "Build the optimized release binary"
task release:
    cargo build --release

@group build
@desc "fmt --check + clippy -D warnings (incl. the seed binary)"
task lint:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings

@group build
@desc "Type-check without codegen"
task check:
    cargo check

# ── Local development ────────────────────────────────────────

# Seed a fresh DB directly (fast, no running server needed), then start the
# server on it. Bumps to the next free port if PORT is taken. `jake pkg.dev port=4000`.
@group dev
@desc "Seed a fresh DB, then start the registry (cargo) (params: port=3000)"
task dev port="3000": [reset]
    @needs cargo
    start={{port}}; p=$start; \
    while lsof -iTCP:$p -sTCP:LISTEN -t >/dev/null 2>&1; do p=$((p+1)); done; \
    base="http://localhost:$p"; \
    [ "$p" = "$start" ] || echo "Port $start is busy — using $p instead."; \
    echo "Seeding a fresh dev database..."; \
    BASE_URL="$base" cargo run --features seed --bin seed -- --fresh; \
    echo "Starting sema-pkg on $base (Ctrl-C to stop)..."; \
    PORT="$p" BASE_URL="$base" cargo run

@group dev
@desc "Start the registry without resetting or seeding"
task run:
    cargo run

# OpenTelemetry request+DB tracing to a JSONL file. Each span
# (request → handler → query) is one line in the file.
@group dev
@desc "Run the registry with OpenTelemetry tracing to a file (params: out=traces.jsonl)"
task trace out="traces.jsonl":
    @needs cargo
    OTEL_TRACES_EXPORTER=file OTEL_TRACE_FILE={{out}} cargo run --release

@group dev
@desc "Seed a fresh DB, build + start in Docker, then tail logs (params: port=3000)"
task docker port="3000": [reset]
    @needs docker
    @needs cargo
    docker compose down 2>/dev/null || true
    start={{port}}; p=$start; \
    while lsof -iTCP:$p -sTCP:LISTEN -t >/dev/null 2>&1; do p=$((p+1)); done; \
    base="http://localhost:$p"; \
    [ "$p" = "$start" ] || echo "Host port $start is busy — using $p instead."; \
    echo "Seeding ./data before the container opens it..."; \
    DATABASE_URL="sqlite://data/registry.db?mode=rwc" BLOB_DIR="data/blobs" BASE_URL="$base" \
      cargo run --features seed --bin seed -- --fresh; \
    PORT="$p" BASE_URL="$base" docker compose up --build -d; \
    echo "Registry running in Docker at $base — 'jake pkg.down' to stop it."; \
    PORT="$p" docker compose logs -f

@group dev
@desc "Stop the Docker registry"
task down:
    docker compose down

# ── Seed / database ──────────────────────────────────────────

# Engine-portable seeder (src/bin/seed.rs). Talks to DATABASE_URL directly, so it
# works on SQLite/Postgres/MySQL and needs no running server. `--fresh` wipes first.
@group db
@desc "Seed realistic dev data into a fresh DB (small set)"
task seed:
    @needs cargo
    cargo run --features seed --bin seed -- --fresh

@group db
@desc "Seed bulk stress data into a fresh DB (~1k users, ~500 packages)"
task seed-stress:
    @needs cargo
    cargo run --features seed --bin seed -- --fresh --large

# Sitemap/feed scale test. Overrides the --huge preset's package count to a full
# 10M so the sitemap index (200 child chunks) and the feeds can be load tested.
# Release build + deferred index rebuild keep the load bounded. Slow — minutes.
@group db
@desc "Seed ~10M packages for sitemap/feed load testing"
task seed-10m:
    @needs cargo
    SEED_PACKAGES=10000000 cargo run --release --features seed --bin seed -- --fresh --huge

@group db
@desc "Delete the local DB + blobs so the next seed is fresh"
task reset:
    rm -f data/registry.db data/registry.db-wal data/registry.db-shm
    rm -rf data/blobs

@group db
@desc "Remove local + e2e databases and blobs"
task clean: [reset]
    rm -f e2e/e2e-test.db*
    rm -rf e2e/e2e-blobs

# ── Tests ────────────────────────────────────────────────────

@group test
@desc "Run the Rust test suite"
task test:
    cargo test

@group test
@desc "Playwright end-to-end tests (params: headed=--headed for a visible browser)"
task e2e headed="":
    @needs npx
    @cd e2e
    npx playwright test {{headed}}

# Concurrent latency test over every endpoint. Point BASE_URL at a running,
# seeded server (start it with RATE_LIMIT_ENABLED=false so the limiter does not
# reject the load). Tune with LOADTEST_CONCURRENCY / LOADTEST_DURATION / etc.
@group test
@desc "Concurrent endpoint latency load test (params: url=http://localhost:3000)"
task loadtest url="http://localhost:3000":
    @needs cargo
    BASE_URL={{url}} cargo run --release --features loadtest --bin loadtest

# One param'd recipe replaces test-sqlite/postgres/mysql. SQLite is file-based
# (no server container); Postgres/MySQL get their server upped and stopped.
@group test
@desc "Test against one DB driver in Docker (params: driver=sqlite|postgres|mysql)"
task driver driver="sqlite":
    @needs docker
    [ "{{driver}}" = "sqlite" ] || docker compose -f docker-compose.test.yml up -d {{driver}} minio minio-create-bucket
    docker compose -f docker-compose.test.yml run --build --rm test-{{driver}}
    [ "{{driver}}" = "sqlite" ] || docker compose -f docker-compose.test.yml stop {{driver}} minio

@group test
@desc "Run the suite against SQLite + Postgres + MySQL (Docker)"
task all-drivers:
    @needs docker
    docker compose -f docker-compose.test.yml up -d postgres mysql minio minio-create-bucket
    docker compose -f docker-compose.test.yml run --build --rm test-sqlite
    docker compose -f docker-compose.test.yml run --build --rm test-postgres
    docker compose -f docker-compose.test.yml run --build --rm test-mysql
    docker compose -f docker-compose.test.yml down -v

# ── Deploy ───────────────────────────────────────────────────

@group deploy
@desc "Deploy to fly.io (see fly.toml)"
task deploy:
    @needs flyctl "install the Fly CLI: https://fly.io/docs/flyctl/install/"
    @confirm "Deploy sema-pkg to fly.io production?"
    flyctl deploy
