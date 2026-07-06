# Jakefile — sema-pkg (self-hostable Sema package registry).
#
# `@rooted` so the sema-lisp/workspace meta-repo can `@import "pkg/Jakefile" as pkg`
# and run `pkg.build` / `pkg.test` from the workspace root. Thin wrappers over the
# existing Makefile targets (see `make help` for the full set: dev, seed, docker).
@rooted

@group pkg
@desc "Build the registry (debug)"
task build:
    cargo build

@group pkg
@desc "Run the Rust test suite"
task test:
    cargo test

@group pkg
@desc "fmt --check + clippy -D warnings"
task lint:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings

@group pkg
@desc "Start the registry locally on a fresh, seeded DB (delegates to make dev)"
task dev:
    make dev

@group pkg
@desc "Deploy to fly.io (see fly.toml)"
task deploy:
    @needs flyctl "install the Fly CLI: https://fly.io/docs/flyctl/install/"
    @confirm "Deploy sema-pkg to fly.io production?"
    flyctl deploy
