# API reference prototype

This is a runnable design and extraction proof for release-level package API
references. It uses the real `sema-retry` 0.1.0 tarball downloaded from the
live registry. It does not change the registry server.

```bash
cd prototypes/api-reference
node extract.mjs
node build.mjs
node verify.mjs
node verify-ast.mjs
node server.mjs
```

Open these views after starting the server:

- `http://127.0.0.1:4176/packages/sema-retry/0.1.0/api` — registry-styled browser view
- `http://127.0.0.1:4176/packages/sema-retry/0.1.0/api/print` — self-contained light page for print or offline save
- `http://127.0.0.1:4176/packages/sema-retry/0.1.0/api.md` — Markdown from the same generated data

The source fixture generator is intentionally limited. Its purpose is to prove
the view contract and show exact data provenance. The production design in
[`API_REFERENCE_PROPOSAL.md`](API_REFERENCE_PROPOSAL.md) requires the Sema
reader's AST and spans for all release sources.

`generated/sema-retry-0.1.0-api.html`,
`generated/sema-retry-0.1.0-api-print.html`, and
`generated/sema-retry-0.1.0-api.md` are standalone review artifacts. They are
materialized from the same `generated/api.json` as the live routes.

The fixture is the tarball downloaded from the live registry, not the local
`pkg-packages` checkout. `metadata.json` records its registry checksum, and
`extract.mjs` refuses to build when the downloaded archive does not match it.
`sema-ast.json` is independent parser evidence produced by
`sema ast --json` from the extracted release source; `verify-ast.mjs` compares
it with the prototype document.

See [`PROVENANCE.md`](PROVENANCE.md) for the registry metadata, archive, and
parser-evidence chain.
