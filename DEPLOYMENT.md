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

## Production checklist

- [ ] `BASE_URL` is your real `https://` origin (enables `Secure` cookies).
- [ ] Behind a TLS-terminating reverse proxy (Caddy/nginx) or Fly's `force_https`.
- [ ] Unique `OAUTH_TOKEN_KEY` if GitHub OAuth is enabled (the server refuses to
      boot with the default key otherwise).
- [ ] Durability for **both** DB and blobs (Litestream+R2, or managed DB + R2).
- [ ] First admin seeded (`seed.sh` — the first admin can't be created via the API).
- [ ] Backups verified by actually restoring once.

### Known gaps

The registry has no built-in request rate limiting, graceful-shutdown drain, or a
deep `/healthz` (it returns `ok` without checking the DB). Put a rate limiter at
the proxy/CDN layer for a public instance. See the issue tracker for the hardening
roadmap.
