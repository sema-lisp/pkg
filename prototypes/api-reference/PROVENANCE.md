# Fixture provenance

The prototype uses the published `sema-retry` 0.1.0 release, not the working
tree in `pkg-packages`.

| Field | Value |
| --- | --- |
| Registry metadata | `GET /api/v1/packages/sema-retry` |
| Download | `GET /api/v1/packages/sema-retry/0.1.0/download` |
| Published | 2026-07-07 21:28:14 |
| Archive size | 8,115 bytes |
| Archive SHA-256 | `ea1f177349994e1cfc0684dfc796063ac389a583448f6b1a574c9b4595dcf82e` |
| AST evidence | `sema 1.37.0 ast --json` |

The checked-in archive contains only `sema.toml`, `LICENSE`, `CHANGELOG.md`,
`package.sema`, `README.md`, and `tests.sema`. The fixture extracts only the
manifest, entrypoint, and README after checking the archive's list. Their hashes
are in [`fixture-release/metadata.json`](fixture-release/metadata.json).

`node extract.mjs` checks the registry archive hash and every extracted-file
hash before building `generated/api.json`. `node verify-ast.mjs` compares that
document with the committed `sema ast --json` output from the same extracted
`package.sema`: all nine exports, signatures, and literal docstrings match.

The present local `pkg-packages/sema-retry/package.sema` has the same source
SHA-256 as this release. That comparison is evidence only; it is never the
prototype's input.
