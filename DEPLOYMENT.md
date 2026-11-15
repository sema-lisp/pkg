# Deploying sema-pkg

`sema-pkg` is a single Rust binary. It needs two pieces of durable state:

1. **The database** — package metadata, users, tokens, audit log. SQLite,
   PostgreSQL, or MySQL, chosen by the `DATABASE_URL` scheme.
2. **The blobs** — package tarballs. Local filesystem (`BLOB_DIR`) or an
   S3-compatible bucket (`BLOB_S3_*`, works with Tigris / Cloudflare R2 / AWS S3
   / MinIO).

Making both durable is the whole deployment decision. Pick a path:

| Path | Best for | DB | Blobs | Backup |
|------|----------|----|-------|--------|
| **[A — Fly.io](#path-a--flyio-recommended)** | a durable public instance, fast | SQLite on a volume | Tigris | Litestream → Tigris, built in |
| **[B — Docker Compose](#path-b--self-host-with-docker-compose)** | your own VM | SQLite | R2 / S3 / Tigris | Litestream sidecar |
| **[C — Bare binary](#path-c--bare-binary--systemd)** | a small box, no Docker | SQLite | filesystem or S3 | Litestream (optional) |
| **[D — Horizontal scale](#path-d--horizontal-scale-managed-postgres--s3)** | many replicas | managed Postgres/MySQL | S3 | managed DB backups |

Paths A–C are single-writer SQLite (one machine). For multiple replicas behind a
load balancer, use Path D.

---

## Path A — Fly.io (recommended)

The fastest way to a durable public registry. One machine runs the binary with
SQLite on a persistent volume; tarballs go to **Tigris** (Fly's S3-compatible
object storage, zero egress); and **Litestream runs inside the same container**,
continuously streaming the SQLite WAL to Tigris. The machine holds no
irreplaceable state — if the volume is lost, the DB is restored from Tigris on
next boot. The `fly.toml`, `Dockerfile`, `litestream.yml`, and `entrypoint.sh`
in this repo do all of it.

```bash
# 1. Create the app + volume (region + size match [mounts] in fly.toml)
fly auth login
fly apps create sema-pkg
fly volumes create sema_pkg_data --region ams --size 3

# 2. Provision Tigris — this injects BUCKET_NAME + AWS_* secrets automatically,
#    which the registry uses for blobs AND Litestream uses for DB backup.
fly storage create

# 3. GitHub OAuth (optional) + a unique 32-byte token-encryption key.
#    The server refuses to boot with the default OAUTH_TOKEN_KEY once OAuth is on.
fly secrets set OAUTH_TOKEN_KEY=$(openssl rand -hex 16) \
  GITHUB_CLIENT_ID=… GITHUB_CLIENT_SECRET=…

# 4. Point BASE_URL at your origin (in fly.toml). Use https://sema-pkg.fly.dev
#    until a custom domain's DNS + cert are in place, so links + Secure cookies
#    are correct. For a custom domain:  fly certs add pkg.example.com

# 5. Deploy, smoke-test, and create the first admin.
fly deploy
fly ssh console -C "sema-pkg doctor"                 # checks DB, blobs, secrets
fly ssh console -C "sema-pkg admin create you you@example.com 'a-strong-password'"
```

That's it — the registry runs migrations on boot, serves on `https://…`, and
Litestream begins replicating. See [Admin CLI](#admin-cli) for what else you can
do, and [Backup & recovery](#backup--recovery-litestream) for how the backup
works and how to restore.

**Cost:** a `shared-cpu-1x` machine that scales to zero when idle, a 3 GB volume,
and Tigris storage (a few cents/GB, zero egress). Order of a few USD/month for a
small registry.

---

## Path B — Self-host with Docker Compose

One VM, SQLite, blobs in an S3-compatible bucket, and Litestream as a sidecar
container replicating the DB to the same bucket. `docker-compose.prod.yml` is
ready to go; it needs an `.env`:

```bash
# .env
BASE_URL=https://pkg.example.com
BLOB_S3_BUCKET=sema-packages
BLOB_S3_ENDPOINT=https://<account-id>.r2.cloudflarestorage.com   # see storage setup
R2_ACCESS_KEY_ID=…
R2_SECRET_ACCESS_KEY=…
OAUTH_TOKEN_KEY=$(openssl rand -hex 16)     # only if using GitHub OAuth
```

```bash
docker compose -f docker-compose.prod.yml up -d
docker compose -f docker-compose.prod.yml exec registry \
  sema-pkg admin create you you@example.com 'a-strong-password'
```

The registry container and the Litestream sidecar share the `registry-data`
volume; Litestream reads the WAL and streams it to `s3://…/litestream/`. Blobs
and the DB backup can share one bucket (blobs under `ab/…`, the DB under
`litestream/`) or use two — your call.

Put a TLS-terminating reverse proxy (Caddy, nginx, or Fly/Cloudflare in front)
on `:3000`, health-checking `/readyz`.

---

## Path C — Bare binary + systemd

No Docker: just the binary and a config file. Smallest possible footprint.

```bash
# Build (or grab a release binary) and install it
cargo build --release
install -m755 target/release/sema-pkg /usr/local/bin/sema-pkg
```

Configure with an env file (all keys optional except where noted — see
`.env.example` for the full list):

```ini
# /etc/sema-pkg.env
HOST=0.0.0.0
PORT=3000
BASE_URL=https://pkg.example.com          # required for correct links + Secure cookies
DATABASE_URL=sqlite:///var/lib/sema-pkg/registry.db?mode=rwc
BLOB_DIR=/var/lib/sema-pkg/blobs          # or set BLOB_S3_* to use object storage
RUST_LOG=info
# OAUTH_TOKEN_KEY=<32 bytes>              # only if using GitHub OAuth
```

```ini
# /etc/systemd/system/sema-pkg.service
[Unit]
Description=sema-pkg registry
After=network-online.target

[Service]
EnvironmentFile=/etc/sema-pkg.env
ExecStart=/usr/local/bin/sema-pkg
Restart=on-failure
# Drain in-flight requests on stop (the binary handles SIGTERM gracefully).
KillSignal=SIGTERM
TimeoutStopSec=30
StateDirectory=sema-pkg                   # creates/owns /var/lib/sema-pkg

[Install]
WantedBy=multi-user.target
```

```bash
systemctl enable --now sema-pkg
sema-pkg doctor                           # DATABASE_URL etc. from the same env
sema-pkg admin create you you@example.com 'a-strong-password'
```

**Backup (recommended):** point Litestream at the SQLite file. Install the
[Litestream binary](https://litestream.io/install/), drop the `litestream.yml`
from this repo at `/etc/litestream.yml` (set `LITESTREAM_BUCKET`,
`LITESTREAM_ENDPOINT`, `LITESTREAM_ACCESS_KEY_ID`, `LITESTREAM_SECRET_ACCESS_KEY`
in the environment), and run it as its own service supervising the binary:

```bash
ExecStart=/usr/local/bin/litestream replicate -exec /usr/local/bin/sema-pkg
```

Without object storage at all, blobs live under `BLOB_DIR` and the DB under
`DATABASE_URL` — back up that one directory (e.g. nightly `restic`/`rsync`) and
you're covered, at the cost of coarser recovery granularity than Litestream.

---

## Path D — Horizontal scale: managed Postgres + S3

When one node isn't enough, point `DATABASE_URL` at a managed PostgreSQL (Neon,
Supabase, RDS, Fly Postgres) and keep blobs in S3. The binary is stateless in
this configuration, so run **N** replicas behind a load balancer with zero shared
disk. No Litestream — the managed DB handles its own backups.

```bash
DATABASE_URL="postgres://user:pass@db.host:5432/sema"
BASE_URL="https://pkg.example.com"
BLOB_S3_BUCKET=sema-packages
BLOB_S3_ENDPOINT=https://<account-id>.r2.cloudflarestorage.com   # omit for AWS S3
BLOB_S3_REGION=auto
BLOB_S3_ACCESS_KEY_ID=…
BLOB_S3_SECRET_ACCESS_KEY=…
```

MySQL works identically (`mysql://…`). All raw SQL uses `?` placeholders that
SeaORM rebinds per backend, so every engine runs the same code path.

---

## Object storage setup

The blob store and the Litestream backup both speak the S3 API. Any of these
work; set `BLOB_S3_*` for blobs (and the matching `LITESTREAM_*` for backup).

### Tigris (Fly.io)

`fly storage create` provisions a bucket and sets `BUCKET_NAME`,
`AWS_ENDPOINT_URL_S3`, `AWS_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`
as app secrets. The registry auto-detects these (no manual `BLOB_S3_*` needed),
and Litestream uses them too. Nothing else to configure.

### Cloudflare R2

1. Create a bucket (e.g. `sema-packages`) in the Cloudflare dashboard.
2. Create an R2 API token (Account → R2 → Manage API Tokens) with **Object Read
   & Write** on that bucket.
3. Set `BLOB_S3_ACCESS_KEY_ID` / `BLOB_S3_SECRET_ACCESS_KEY` to the token's keys,
   `BLOB_S3_ENDPOINT=https://<account-id>.r2.cloudflarestorage.com`, and
   `BLOB_S3_REGION=auto` (R2 has no regions). R2 has zero egress fees.

### AWS S3

Set `BLOB_S3_BUCKET`, `BLOB_S3_REGION` (a real region, e.g. `us-east-1`), and the
access key / secret. **Omit `BLOB_S3_ENDPOINT`** — the AWS default is used.

### MinIO (self-hosted S3)

Set `BLOB_S3_ENDPOINT=http://minio:9000` (the registry auto-detects `http://` and
allows plaintext for local MinIO), plus bucket, region, and the access key /
secret.

### Migrating filesystem blobs to S3

Blobs are content-addressed and sharded identically on both backends
(`ab/<sha256>.tar.gz`), so migration is a straight copy — no re-hashing:

```bash
rclone copy ./data/blobs r2:sema-packages    # rclone configured for your remote
```

Then set `BLOB_S3_*` and restart. Reads don't fall through between backends, so
copy everything before cutting over.

---

## Backup & recovery (Litestream)

[Litestream](https://litestream.io) continuously streams the SQLite WAL to
object storage — near-real-time backup with point-in-time restore, no cron, no
`.dump`. It only applies to SQLite (Path A–C); Postgres/MySQL (Path D) use the
managed DB's own backups. WAL mode is enabled by the registry automatically.

**How it's wired in this repo:**

- **Fly (Path A):** the container's entrypoint runs
  `litestream replicate -exec sema-pkg` when `LITESTREAM_REPLICATE=1` (set in
  `fly.toml`), so Litestream supervises the app in one process and does a final
  flush when the app drains on stop. `kill_timeout = "10s"` gives that flush
  headroom.
- **Compose (Path B):** Litestream runs as a separate sidecar container
  (`command: replicate`) sharing the data volume.
- **Bare binary (Path C):** run `litestream replicate -exec sema-pkg` yourself.

All three read the same `litestream.yml`, which keeps **7 days** of history with
a daily snapshot (tune the root `snapshot.retention` / `snapshot.interval`).

**Restore** rebuilds the DB from the latest replica. On Fly this happens
automatically when the volume is empty (the entrypoint runs
`litestream restore -if-replica-exists` before starting, and never overwrites an
existing DB). To restore manually:

```bash
litestream restore -config /etc/litestream.yml /data/registry.db
```

**Verify your backup once** — an untested backup isn't a backup. Restore to a
scratch path and boot the registry against it (from anywhere with the bucket
credentials, e.g. your laptop):

```bash
# 1. Restore the latest replica to a scratch DB (env: LITESTREAM_BUCKET,
#    LITESTREAM_ENDPOINT, LITESTREAM_ACCESS_KEY_ID, LITESTREAM_SECRET_ACCESS_KEY).
litestream restore -config litestream.yml -o /tmp/drill.db /data/registry.db

# 2. Boot the registry against it and check it reads the data.
DATABASE_URL="sqlite:///tmp/drill.db?mode=rwc" BLOB_DIR=/tmp/drill-blobs \
  BASE_URL=http://localhost:8080 sema-pkg stats     # user/package counts
DATABASE_URL="sqlite:///tmp/drill.db?mode=rwc" sema-pkg doctor
```

The counts should match the live instance.

**Caveat — one writer only.** SQLite + Litestream is single-node. Don't run more
than one registry replica against the same database file; concurrent writers
corrupt it. If you need horizontal scale, use Path D.

---

## Admin CLI

`sema-pkg <command>` runs management tasks directly against `DATABASE_URL` —
no HTTP, no raw SQL, works on any backend. Run it wherever the binary and env
live: `fly ssh console -C "sema-pkg …"` (Path A),
`docker compose … exec registry sema-pkg …` (Path B), or just `sema-pkg …`
(Path C).

**First admin** (the API can't create it — chicken-and-egg):

```bash
sema-pkg admin create <username> <email> <password>   # new admin user
sema-pkg admin promote <username>                      # or promote a web-registered user
```

**User administration:**

```bash
sema-pkg admin list                          # list admins
sema-pkg admin demote <username>             # revoke admin role
sema-pkg admin ban <username>                # ban (also revokes tokens + sessions)
sema-pkg admin unban <username>
sema-pkg admin reset-password <username> <new-password>
sema-pkg admin token-create <username> [name]  # mint an API token (printed once)
sema-pkg admin revoke-tokens <username>      # invalidate all their API tokens
```

**Package moderation:**

```bash
sema-pkg package yank <name> <version>       # hide a version (stays downloadable by lockfile)
sema-pkg package remove <name>               # delete a package + all versions, reclaim blobs
```

**Diagnostics:**

```bash
sema-pkg doctor    # verify DB, blob store, and required secrets are reachable/valid
sema-pkg stats     # users, packages, banned users, open reports, 30-day downloads
```

Every privileged CLI mutation is written to the audit log (attributed to
`cli:<os-user>`), so admin-console and CLI actions share one trail.

---

## Health checks

Two probes, split along the Kubernetes convention:

- **`GET /healthz`** — liveness. Cheap, dependency-free, always `200 ok` while the
  process is up. Wire this to your orchestrator's *restart* probe.
- **`GET /readyz`** — readiness. Pings the database; `200 {"status":"ready"}` when
  reachable, `503` when not. Wire this to your load balancer's *should-I-route*
  probe so a transient DB outage drains traffic instead of triggering a restart
  loop.

On Fly, `[http_service]` checks `/readyz`. Behind nginx/Caddy, health-check the
upstream on `/readyz`.

## Rate limiting

The registry rate-limits by client IP (GCRA) out of the box — no proxy config
required. Health probes, static assets, and web pages are never limited.
Responses carry `x-ratelimit-*` headers, and 429s carry an actionable
`retry-after` (the CLI honors it with backoff).

| Variable | Default | Description |
|---|---|---|
| `RATE_LIMIT_ENABLED` | `true` | Set `false` only behind a trusted gateway that already rate-limits. |
| `RATE_LIMIT_RPS` | `20` | Sustained requests/sec per IP on the general/write API (publish, search, admin). |
| `RATE_LIMIT_BURST` | `40` | Burst allowance per IP before throttling. |
| `RATE_LIMIT_READ_RPS` | `100` | Sustained requests/sec per IP on the install hot path (package metadata + tarball download). Generous so multi-package installs aren't throttled. |
| `RATE_LIMIT_READ_BURST` | `500` | Burst allowance per IP on the install hot path. |

The install path (metadata + download) is deliberately on a separate, generous
tier: resolving one project pulls many packages in a burst from a single IP, so
sharing the strict general limit would 429 legitimate installs. Publishing,
search, and admin stay on the tighter general tier.

**Behind a reverse proxy**, the limiter keys on `X-Forwarded-For` / `X-Real-IP` /
`Forwarded`. Make sure your proxy sets one (Caddy and Fly do by default; for
nginx add `proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;`) —
otherwise every request keys to the proxy's IP and shares one bucket.

## Graceful shutdown

On `SIGINT`/`SIGTERM` (what Docker, Kubernetes, systemd, and Fly send on stop) the
server stops accepting new connections and drains in-flight requests before
exiting. Give the orchestrator enough grace time — `stop_grace_period` in the
compose files, `kill_timeout` in `fly.toml`, `TimeoutStopSec` in systemd — so the
drain (and Litestream's final flush) completes.

## Observability

Traces and metrics are compiled into the binary and **off until configured** —
no exporter runs and `/metrics` is not served unless you opt in. Log verbosity
(`RUST_LOG`) and trace capture (`OTEL_LOG`) are independent, so you can run quiet
logs with rich traces.

### Traces (OpenTelemetry)

| Variable | Values | Meaning |
|---|---|---|
| `OTEL_TRACES_EXPORTER` | `none` (default), `file`, `otlp` | Where spans go |
| `OTEL_TRACE_FILE` | path (default `traces.jsonl`) | File exporter target |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | e.g. `http://jaeger:4317` | OTLP/gRPC collector |
| `OTEL_SERVICE_NAME` | default `sema-pkg` | Service name in the tracing UI |
| `OTEL_TRACES_SAMPLER_ARG` | `0.0`–`1.0` (default `1.0`) | Head sampling ratio |
| `OTEL_LOG` | tracing filter | Which spans are captured |

Point `otlp` at Jaeger, Grafana Tempo, or an OpenTelemetry Collector (all accept
OTLP/gRPC on `:4317`). In production, sample: `OTEL_TRACES_SAMPLER_ARG=0.05`.

### Metrics (Prometheus)

`METRICS_ENABLED=true` serves Prometheus metrics at `GET /metrics`
(unauthenticated — scrape it on a private network or behind the proxy):

- **HTTP RED**, per matched route: `http_requests_total`,
  `http_request_duration_seconds` (histogram), `http_requests_in_flight`.
- **Process**: `process_resident_memory_bytes`, `process_cpu_seconds_total`,
  `process_open_fds`, `process_threads`, …
- **Application** (refreshed every 15s): `sema_packages_total`,
  `sema_users_total`, `sema_users_banned`, `sema_reports_open`,
  `sema_downloads_30d`, and `sema_build_info{version}`.

To carry metrics into an OTLP pipeline, point an OpenTelemetry Collector's
`prometheus` receiver at `/metrics`.

### Local stack

`docker-compose.observability.yml` runs an OpenTelemetry Collector (hub) with
Jaeger + Prometheus + Grafana:

```bash
docker compose -f docker-compose.observability.yml up -d
OTEL_TRACES_EXPORTER=otlp OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317 \
  METRICS_ENABLED=true sema-pkg
# Jaeger UI :16686 · Prometheus :9090 · Grafana :3001
```

## Production checklist

- [ ] `BASE_URL` is your real `https://` origin (enables `Secure` cookies).
- [ ] Behind a TLS-terminating reverse proxy (Caddy/nginx) or Fly's `force_https`.
- [ ] Proxy forwards the client IP (`X-Forwarded-For`) so rate limiting keys per client.
- [ ] Readiness probe wired to `/readyz`, liveness to `/healthz`.
- [ ] Unique `OAUTH_TOKEN_KEY` if GitHub OAuth is enabled (the server refuses to
      boot with the default key otherwise).
- [ ] Durability for **both** DB and blobs (Litestream + object storage, or a
      managed DB + object storage).
- [ ] First admin created — `sema-pkg admin create <user> <email> <password>`.
- [ ] Backups verified by actually restoring once.
```
