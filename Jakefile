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
@desc "fmt --check + clippy -D warnings"
task lint:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings

@group build
@desc "Type-check without codegen"
task check:
    cargo check

# ── Local development ────────────────────────────────────────

# Start on a fresh, seeded DB. Bumps to the next free port if PORT is taken and
# points both the server and the seed at it. `jake pkg.dev port=4000`.
@group dev
@desc "Start the registry (cargo) on a fresh DB + seed (params: port=3000)"
task dev port="3000": [reset]
    @needs cargo
    start={{port}}; p=$start; \
    while lsof -iTCP:$p -sTCP:LISTEN -t >/dev/null 2>&1; do p=$((p+1)); done; \
    base="http://localhost:$p"; \
    [ "$p" = "$start" ] || echo "Port $start is busy — using $p instead."; \
    echo "Starting sema-pkg on $base — seeding once it's healthy (Ctrl-C to stop)..."; \
    SEED_MODE=local BASE_URL="$base" bash seed.sh --wait & \
    PORT="$p" BASE_URL="$base" cargo run

@group dev
@desc "Start the registry without resetting or seeding"
task run:
    cargo run

@group dev
@desc "Build + start in Docker on a fresh DB + seed, then tail logs (params: port=3000)"
task docker port="3000": [reset]
    @needs docker
    docker compose down 2>/dev/null || true
    start={{port}}; p=$start; \
    while lsof -iTCP:$p -sTCP:LISTEN -t >/dev/null 2>&1; do p=$((p+1)); done; \
    base="http://localhost:$p"; \
    [ "$p" = "$start" ] || echo "Host port $start is busy — using $p instead."; \
    PORT="$p" BASE_URL="$base" docker compose up --build -d; \
    SEED_MODE=docker BASE_URL="$base" bash seed.sh --wait; \
    echo "Registry running in Docker at $base — 'jake pkg.down' to stop it."; \
    PORT="$p" docker compose logs -f

@group dev
@desc "Stop the Docker registry"
task down:
    docker compose down

# ── Seed / database ──────────────────────────────────────────

@group db
@desc "Seed a registry that is already running (no reset)"
task seed:
    BASE_URL="${BASE_URL:-http://localhost:3000}" bash seed.sh

@group db
@desc "Seed, then bulk-load synthetic data (local SQLite only)"
task seed-stress: [seed]
    python3 seed_stress.py

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

# One param'd recipe replaces test-sqlite/postgres/mysql. SQLite is file-based
# (no server container); Postgres/MySQL get their server upped and stopped.
@group test
@desc "Test against one DB driver in Docker (params: driver=sqlite|postgres|mysql)"
task driver driver="sqlite":
    @needs docker
    [ "{{driver}}" = "sqlite" ] || docker compose -f docker-compose.test.yml up -d {{driver}}
    docker compose -f docker-compose.test.yml run --rm test-{{driver}}
    [ "{{driver}}" = "sqlite" ] || docker compose -f docker-compose.test.yml stop {{driver}}

@group test
@desc "Run the suite against SQLite + Postgres + MySQL (Docker)"
task all-drivers:
    @needs docker
    docker compose -f docker-compose.test.yml up -d postgres mysql
    docker compose -f docker-compose.test.yml run --rm test-sqlite
    docker compose -f docker-compose.test.yml run --rm test-postgres
    docker compose -f docker-compose.test.yml run --rm test-mysql
    docker compose -f docker-compose.test.yml down -v

# ── Deploy ───────────────────────────────────────────────────

@group deploy
@desc "Deploy to fly.io (see fly.toml)"
task deploy:
    @needs flyctl "install the Fly CLI: https://fly.io/docs/flyctl/install/"
    @confirm "Deploy sema-pkg to fly.io production?"
    flyctl deploy
