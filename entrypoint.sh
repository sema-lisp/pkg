#!/bin/sh
# Container entrypoint. Two modes:
#
#  - Default: exec the registry directly (`sema-pkg`). Used by the
#    docker-compose recipe, where Litestream runs as a *separate* sidecar
#    container, and by anyone who doesn't want built-in backup.
#
#  - LITESTREAM_REPLICATE=1: run Litestream in this same container, supervising
#    the registry (`litestream replicate -exec sema-pkg`). Used on Fly.io, which
#    is single-container. Litestream streams the SQLite WAL to S3 and does a
#    final flush when the app drains on SIGTERM.
set -e

DB="${DB_PATH:-/data/registry.db}"

if [ "${LITESTREAM_REPLICATE:-0}" = "1" ]; then
  # Map provider env → the vars litestream.yml expands. On Fly, `fly storage
  # create` injects the Tigris BUCKET_NAME / AWS_ENDPOINT_URL_S3 / AWS_* creds;
  # fall back to those when the LITESTREAM_* names aren't already set (compose
  # sets them directly).
  export LITESTREAM_BUCKET="${LITESTREAM_BUCKET:-$BUCKET_NAME}"
  export LITESTREAM_ENDPOINT="${LITESTREAM_ENDPOINT:-$AWS_ENDPOINT_URL_S3}"
  export LITESTREAM_ACCESS_KEY_ID="${LITESTREAM_ACCESS_KEY_ID:-$AWS_ACCESS_KEY_ID}"
  export LITESTREAM_SECRET_ACCESS_KEY="${LITESTREAM_SECRET_ACCESS_KEY:-$AWS_SECRET_ACCESS_KEY}"

  if [ ! -f "$DB" ]; then
    # Fresh or lost volume: rebuild from the latest replica. `-if-replica-exists`
    # is a no-op on the very first deploy (nothing backed up yet). The file-exists
    # guard means an existing live DB is never overwritten.
    echo "litestream: no local DB at $DB — restoring from replica if one exists"
    litestream restore -if-replica-exists "$DB"
  fi

  echo "litestream: replicating $DB → ${LITESTREAM_BUCKET} (${LITESTREAM_ENDPOINT})"
  exec litestream replicate -exec "sema-pkg"
fi

exec sema-pkg
