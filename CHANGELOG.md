# Changelog

All notable changes to sema-pkg are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

Production-readiness pass: durability, observability, operability, and scale.

New database migrations (m007–m009) run automatically on startup; no manual
steps required.

### Added

- **README rendering for uploaded packages.** On publish, the registry now
  extracts `README.md` from the package tarball and renders it on the package
  page (previously only GitHub-linked packages showed a README). Code blocks are syntax-highlighted (syntect); `sema` fences use a Lisp grammar. Adds
  `sema-pkg package backfill-readmes` to populate it for packages published
  before this. The official badge is now a single aligned verified-seal icon
  (no misaligned pill).
- **`sema-pkg admin token-create <user> [name]`** — mint an API token for a user
  from the operator CLI (no browser login), so a bot/house account can publish
  without a web session. Prints the token once; audit-logged.
- **Built-in Litestream backup on Fly.io.** The container can now run Litestream
  in-process (`LITESTREAM_REPLICATE=1`), continuously streaming the SQLite WAL to
  Tigris and restoring automatically onto a fresh volume — so the machine holds
  no irreplaceable state. `litestream.yml` is provider-agnostic (env-driven
  bucket/endpoint), so the same config serves Tigris, R2, and S3; the
  docker-compose sidecar path still works. Deployment docs rewritten around
  Fly.io (recommended), Docker, and bare-binary + systemd, with an object-storage
  matrix (Tigris/R2/S3/MinIO), a backup/restore section, and an admin CLI
  reference.
- **S3-compatible blob storage** (Cloudflare R2, MinIO, AWS S3) alongside the
  filesystem backend, selected by `BLOB_S3_BUCKET` — decouples tarball
  durability from the compute node for stateless / multi-instance deploys.
- **Rate limiting** — IP-keyed (GCRA), honouring `X-Forwarded-For` behind a
  proxy: a generous global tier plus a stricter tier on auth endpoints.
  Configurable via `RATE_LIMIT_*`, on by default.
- **Health probes** — `/healthz` (liveness) and `/readyz` (readiness, pings the
  DB and returns 503 when unreachable).
- **Graceful shutdown** — drains in-flight requests on SIGINT/SIGTERM.
- **Observability** (built in, no-op until configured):
  - OpenTelemetry traces to a JSONL file or OTLP (Jaeger / Grafana Tempo /
    Collector), with head sampling.
  - Prometheus metrics at `/metrics` — request RED, process, and application
    gauges (`METRICS_ENABLED=true`).
  - `docker-compose.observability.yml` (Collector + Jaeger + Prometheus +
    Grafana).
- **Operator CLI** — `sema-pkg admin|package|stats|doctor`: create the first
  admin, promote/demote/ban/unban, reset-password, revoke-tokens, yank/remove
  packages, print stats, and a `doctor` deployment smoke test. No manual DB
  edits.
- **Engine-portable dev seeder** (`--features seed`) replacing the old
  `seed.sh` / `seed_stress.py`: realistic data on SQLite/Postgres/MySQL, real
  logins and blobs, and a `--huge` preset scaling to 10M packages.
- **Concurrent load-test tool** (`--features loadtest`) covering every endpoint.
- **Full-text search** — SQLite FTS5 (with prefix indexes), Postgres `pg_trgm`,
  MySQL `FULLTEXT`; exact-name matches rank first.
- `DATABASE_MAX_CONNECTIONS` to tune the connection pool.
- **Editable profile** — the account page now has a Save button that persists
  email and homepage (`PUT /api/v1/account`).
- **Official badge** — packages owned by the `sema` house account render a
  verified "Official" badge on their detail page.
- **Tigris / AWS-style env fallback** — S3 blob config auto-detects the
  `BUCKET_NAME` / `AWS_*` variables injected by Fly Tigris, so no manual
  `BLOB_S3_*` mapping is needed on Fly.io.

### Changed

- Multi-engine raw SQL is unified on `?` placeholders (rewritten to `$N` for
  Postgres) with numeric reads that handle Postgres/MySQL `Decimal`.
- Observability is a default, always-compiled feature rather than opt-in.
- `jake` fully replaces the Makefile.

### Performance

- Indexes for the admin and listing hot paths (m007) — the admin user listing
  went from ~12 s to ~2 ms at 1M packages.
- Cached admin dashboard stats (single-flight, 30 s TTL) plus a covering index
  for the rolling download sum (m009) — ~3.7 s to ~80 ms at 10M packages.
- Homepage recent-packages query rewritten to drop a per-package correlated
  subquery — ~490 ms to ~4 ms on Postgres.
- Search made fast at scale (bounded candidate ranking, capped counts, FTS
  prefix indexes) — a term matching all 10M packages returns in ~1 ms.

### Fixed

- Package removal now reclaims orphaned blobs (dedup-safe: a content-addressed
  blob is only deleted when no remaining version references it). Yank keeps the
  blob.
- Operator CLI mutations (create-admin, promote/demote, ban/unban,
  reset-password, revoke-tokens, yank, remove) now write to the audit trail —
  previously they were invisible in the admin console. Entries are attributed to
  `cli:<os-user>` so CLI actions are accountable alongside web actions.
- Rate limiting no longer throttles installs. The install hot path (package
  metadata + tarball download) now has its own generous tier
  (`RATE_LIMIT_READ_RPS`/`RATE_LIMIT_READ_BURST`, default 100 rps / 500 burst)
  instead of sharing the strict general limit (20/40) — resolving a project with
  many dependencies was tripping the shared per-IP burst and getting 429'd.
  Publishing, search, and admin stay on the tighter general tier.
- Every `429 Too Many Requests` now carries an actionable `Retry-After` header.
  The limiter's sub-second replenish rounded `Retry-After` down to `0`, telling
  clients to retry immediately (and get throttled again); it's now floored at
  1 second so compliant clients back off correctly.
