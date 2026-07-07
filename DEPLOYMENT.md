# Deploying sema-pkg

`sema-pkg` is a single Rust binary. It needs two pieces of durable state:

1. **The database** — package metadata, users, tokens, audit log. SQLite,
   PostgreSQL, or MySQL, chosen by the `DATABASE_URL` scheme.
2. **The blobs** — package tarballs. Local filesystem (`BLOB_DIR`) or an
   S3-compatible bucket (`BLOB_S3_*`).

How you make those two durable is the entire deployment decision. Three paths,
cheapest first.

## Path A — Single node: SQLite + Litestream → R2 (recommended)

One small VM, SQLite on its local disk, and [Litestream](https://litestream.io)
streaming the SQLite WAL to Cloudflare R2 continuously. Tarballs also go to R2 via
`BLOB_S3_*`, so the VM holds **no** irreplaceable state — if it dies, restore the
DB from R2 onto a fresh VM and repoint it at the same bucket.

This is the sweet spot for a self-hosted registry: no managed database bill, R2
has no egress fees, and Litestream gives you point-in-time recovery.

```yaml
# docker-compose.prod.yml — SQLite + Litestream + R2
services:
  registry:
    image: ghcr.io/sema-lisp/sema-pkg:latest   # or build: .
    restart: unless-stopped
    ports:
      - "3000:3000"
    volumes:
      - registry-data:/data
    environment:
      DATABASE_URL: "sqlite:///data/registry.db?mode=rwc"
      BASE_URL: "https://pkg.example.com"
      RUST_LOG: info
      # Blobs in R2 (see the shared R2 credentials below)
      BLOB_S3_BUCKET: sema-packages
      BLOB_S3_ENDPOINT: https://<account-id>.r2.cloudflarestorage.com
      BLOB_S3_REGION: auto
      BLOB_S3_ACCESS_KEY_ID: ${R2_ACCESS_KEY_ID}
      BLOB_S3_SECRET_ACCESS_KEY: ${R2_SECRET_ACCESS_KEY}
      # If using GitHub OAuth, set a unique 32-byte key:
      # OAUTH_TOKEN_KEY: ${OAUTH_TOKEN_KEY}

  litestream:
    image: litestream/litestream:latest
    restart: unless-stopped
    volumes:
      - registry-data:/data
      - ./litestream.yml:/etc/litestream.yml:ro
    environment:
      LITESTREAM_ACCESS_KEY_ID: ${R2_ACCESS_KEY_ID}
      LITESTREAM_SECRET_ACCESS_KEY: ${R2_SECRET_ACCESS_KEY}
    command: replicate

volumes:
  registry-data:
```

```yaml
# litestream.yml
dbs:
  - path: /data/registry.db
    replicas:
      - type: s3
        bucket: sema-packages
        path: litestream/registry.db
        endpoint: https://<account-id>.r2.cloudflarestorage.com
        region: auto
        force-path-style: true
```

The blob bucket and the Litestream backup can be the **same** R2 bucket (blobs
under `ab/…`, the DB replica under `litestream/`) or two buckets — your call.

**Restore after a lost node** (before starting the registry container):

```bash
litestream restore -config litestream.yml /data/registry.db
```

Then bring the stack up — the registry runs its migrations on boot and continues.

### Caveats

- **One writer only.** SQLite + Litestream is single-node. Don't scale the
  `registry` service past one replica — concurrent writers on the same file will
  corrupt it, and Litestream replicates one node's WAL. If you need horizontal
  scale, use Path B.
- Litestream replicates the DB, not the blobs — blobs are durable because they're
  already in R2, not on the VM.
- Snapshot the R2 bucket's lifecycle/retention to bound recovery window and cost.

## Path B — Horizontal scale: managed Postgres + R2 blobs

When one node isn't enough (or you'd rather not babysit SQLite), point
`DATABASE_URL` at a managed PostgreSQL (Neon, Supabase, RDS, Fly Postgres) and
keep blobs in R2. The registry binary is stateless in this configuration, so you
can run **N** replicas behind a load balancer and roll deploys with zero shared
disk.

```bash
DATABASE_URL="postgres://user:pass@db.host:5432/sema"
BASE_URL="https://pkg.example.com"
BLOB_S3_BUCKET=sema-packages
BLOB_S3_ENDPOINT=https://<account-id>.r2.cloudflarestorage.com
BLOB_S3_REGION=auto
BLOB_S3_ACCESS_KEY_ID=...
BLOB_S3_SECRET_ACCESS_KEY=...
```

MySQL works identically (`mysql://…`). All raw SQL uses `?` placeholders that
SeaORM rebinds per backend, so every engine runs the same code path.

## Path C — Fly.io (fastest to stand up)

`fly.toml` in this repo deploys a single machine with a persistent volume for
SQLite + local blobs — no object storage required. Good for a quick public
instance; back up the volume (`fly volumes snapshots`) or add Litestream/R2 as in
Path A for real durability.

```bash
fly launch --copy-config      # first time
fly secrets set OAUTH_TOKEN_KEY=$(openssl rand -base64 24)   # if using OAuth
jake deploy                   # fly deploy, with a confirmation prompt
```

## Cloudflare R2 setup (for Paths A and B)

1. Create a bucket (e.g. `sema-packages`) in the Cloudflare dashboard.
2. Create an R2 API token (Account → R2 → Manage API Tokens) with
   **Object Read & Write** on that bucket.
3. Use the token's Access Key ID / Secret as `BLOB_S3_ACCESS_KEY_ID` /
   `BLOB_S3_SECRET_ACCESS_KEY`, and the account's S3 endpoint
   (`https://<account-id>.r2.cloudflarestorage.com`) as `BLOB_S3_ENDPOINT`.
4. `BLOB_S3_REGION=auto` — R2 has no regions.

The same variables work for AWS S3 (drop `BLOB_S3_ENDPOINT`, set a real region)
and MinIO (`BLOB_S3_ENDPOINT=http://minio:9000`). The registry auto-detects
`http://` endpoints and allows plaintext for local MinIO.

## Migrating filesystem blobs to S3

Blobs are content-addressed and sharded identically on both backends
(`ab/<sha256>.tar.gz`), so migration is a straight copy — no re-hashing:

```bash
# with rclone configured for your R2/S3 remote
rclone copy ./data/blobs r2:sema-packages
```

Then set `BLOB_S3_*` and restart. Reads fall through to whichever backend is
configured; there's no dual-read, so copy everything before cutting over.

## Health checks

Two probes, split along the Kubernetes convention:

- **`GET /healthz`** — liveness. Cheap, dependency-free, always `200 ok` while the
  process is up. Wire this to your orchestrator's *restart* probe.
- **`GET /readyz`** — readiness. Pings the database; `200 {"status":"ready"}` when
  reachable, `503 {"status":"unavailable"}` when not. Wire this to your load
  balancer's *should-I-route-traffic* probe so a transient DB outage drains traffic
  instead of triggering a restart loop.

On Fly, point the `[http_service]` check at `/readyz`. Behind nginx/Caddy, health
check the upstream on `/readyz`.

## Rate limiting

The registry rate-limits by client IP (GCRA) out of the box — no proxy config
required. Two tiers: a generous global limit on the API, and a stricter fixed
limit on `register`/`login` to blunt credential brute-forcing. Health probes,
static assets, and web pages are never limited. Responses carry `x-ratelimit-*`
headers, and 429s carry `retry-after`.

| Variable | Default | Description |
|---|---|---|
| `RATE_LIMIT_ENABLED` | `true` | Set `false` only behind a trusted gateway that already rate-limits. |
| `RATE_LIMIT_RPS` | `20` | Sustained requests/sec per IP on the general API. |
| `RATE_LIMIT_BURST` | `40` | Burst allowance per IP before throttling. |

**Behind a reverse proxy**, the limiter keys on `X-Forwarded-For` / `X-Real-IP` /
`Forwarded`. Make sure your proxy sets one of these (Caddy and Fly do by default;
for nginx add `proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;`) —
otherwise every request keys to the proxy's IP and shares one bucket.

## Graceful shutdown

On `SIGINT`/`SIGTERM` (what Docker, Kubernetes, systemd, and Fly send on stop) the
server stops accepting new connections and drains in-flight requests before
exiting. No special config needed. Give the orchestrator enough grace time — the
`stop_grace_period` in the compose files is set accordingly.

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

To carry metrics into an OTLP pipeline (Grafana, an OTel backend), point an
OpenTelemetry Collector's `prometheus` receiver at `/metrics` — the stack below
does exactly that, so metrics and traces both flow through OTel.

### Local stack

`docker-compose.observability.yml` runs an OpenTelemetry Collector (hub) with
Jaeger + Prometheus + Grafana. The registry sends OTLP traces to the collector
and the collector scrapes its `/metrics`:

```bash
docker compose -f docker-compose.observability.yml up -d
# then run the registry pointed at the collector:
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
- [ ] Durability for **both** DB and blobs (Litestream+R2, or managed DB + R2).
- [ ] First admin created (the first admin can't be created via the API — promote a
      user directly in the DB, e.g. `UPDATE users SET is_admin = 1 WHERE username = '…'`).
- [ ] Backups verified by actually restoring once.
