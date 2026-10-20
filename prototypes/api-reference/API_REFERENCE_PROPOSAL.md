# Package API reference proposal

## Goal

Provide release-specific API references at
`/packages/{name}/{version}/api`, with a plain self-contained print view at
`/packages/{name}/{version}/api/print` and an agent-readable Markdown view at
`/packages/{name}/{version}/api.md`. All three views must render one stored API
document model.

The reference describes only facts that can be extracted from the package
release. It must never infer types, return values, aliases, macro behaviour, or
parameter names from call sites or README prose.

## Reliable inputs

At publish/import time, unpack the immutable release tarball into a temporary,
bounded directory. Read only the manifest's declared entrypoint. Parse it with
the exact `sema-reader` version used by the registry, using
`read_many_with_spans`. Do not use a regular expression or execute package code.

For a literal top-level `(module name ...)` form:

1. Accept only a literal `(export symbol ...)` list in that module.
2. For each exported symbol, inspect direct module-level forms.
3. Record a signature only for one direct `(define (symbol args...) ...)` form.
   Treat its first body string as a docstring only when another body form follows.
   A lone string is a return value, not documentation.
4. Store the source file, start line, release content digest, parser version, and
   extraction result with the package version.

Reject a candidate signature when an exported name has more than one direct
definition, or a direct `(set! name ...)` rebinding in the same module. Mark
that known export `unresolved-export`; do not choose a definition by source
order. If the export form is not a literal list of symbols, no reliable export
names may exist: store an unsupported extraction status and no inferred entries.

The source tree and digest are the provenance. Source links must address the
stored release archive, for example
`/packages/{name}/{version}/source/{path}#L{line}`. If an upstream link is
shown, it must name an immutable commit, never a default branch. The package `README.md` remains
separate: it is rendered as README content and may be linked to an API entry,
but it does not produce an API signature or description.

## Code evidence (pinned)

These links are pinned to monorepo commit
[`24558fb0ad4bf9488b6f398eafea01f7aa025462`](https://github.com/sema-lisp/sema/tree/24558fb0ad4bf9488b6f398eafea01f7aa025462):

- [`read_many_with_spans`](https://github.com/sema-lisp/sema/blob/24558fb0ad4bf9488b6f398eafea01f7aa025462/crates/sema-reader/src/reader.rs#L1005-L1014)
  returns parsed values and a span map.
- The LSP's [AST docstring extractor](https://github.com/sema-lisp/sema/blob/24558fb0ad4bf9488b6f398eafea01f7aa025462/crates/sema-lsp/src/definitions.rs#L502-L548)
  recognizes direct `define` shorthand and treats a leading string as a
  docstring only when later body forms exist.
- The evaluator's [module export registration](https://github.com/sema-lisp/sema/blob/24558fb0ad4bf9488b6f398eafea01f7aa025462/crates/sema-eval/src/eval.rs#L4040-L4063)
  receives a list and requires every declared export name to be a symbol; the
  [collector](https://github.com/sema-lisp/sema/blob/24558fb0ad4bf9488b6f398eafea01f7aa025462/crates/sema-eval/src/special_forms.rs#L651-L675)
  reads the module environment at runtime. This supports a static declaration
  view, not a claim about the final runtime API.

## What the current language data can and cannot prove

The Sema reader exposes parsed values and source spans. The package source
convention uses a literal `module`, `export`, and direct `define` forms; this is
enough to identify the nine `sema-retry` exports, their parameter forms, and
their literal docstrings without evaluating the package.

It cannot safely resolve these cases without a new language-level export
metadata contract:

- exports created by macro expansion or computed at runtime;
- aliases that export a name bound through another expression;
- definitions nested in conditionals, generated modules, or other forms;
- function values defined as `(define name (fn ...))` unless the extractor adds
  a specific, parser-backed rule;
- a duplicate direct definition or direct `set!` rebinding of an exported name;
- types, return types, optional-argument meaning, examples, deprecation, or
  stability labels that only appear in prose.

For unsupported cases with literal export names, publish the name with an
explicit `unresolved-export` status and no invented signature. If names are not
literal, publish only the unsupported extraction status. Extraction errors do
not block a release; they store diagnostics so the package page can show its
normal README and version information.

## Stored release document

Store the generated document beside `package_versions`, keyed by the immutable
version id:

```text
api_document(version_id, schema_version, source_digest, parser_version,
             extraction_status, document_json, generated_at)
```

`document_json` contains package/version provenance, exports, direct-definition
signatures, docstrings, and source spans. The registry does not re-extract on a
web request. Reprocessing is an explicit admin migration, so a page remains
stable even when the reader changes.

## Presentation

The normal page extends the registry's dark, restrained package style. A source
link sits beside every generated API entry. A distinct README disclosure is
labelled as verbatim README content. The print page inlines all CSS and uses no
font, script, image, or stylesheet request, so saving that single response
preserves the reference. Its print CSS avoids splitting an API item across
pages. The `.md` route uses the same model and returns `text/markdown`.

## Validation and rollout

1. Unit-test each accepted and unsupported source shape against `sema-reader`
   fixtures, including comments, multiline docstrings, escaped strings, and
   malformed input.
2. Add golden JSON documents for a small set of published packages, including
   `sema-retry`.
3. Assert HTML, print HTML, and Markdown are generated from identical document
   fields. Check that every entry has a source span or `unresolved-export`.
4. Compare golden documents with `sema ast --json` output from the same archive
   as an independent fixture check. This proves agreement on source
   declarations, not the dynamic runtime API. The production implementation
   still uses `sema-reader` directly, including its spans.
5. Roll out behind a config flag. Backfill only releases with stored blobs;
   GitHub redirect-only versions need immutable source snapshots first.

This prototype's `extract.mjs` is deliberately narrow and is not a production
parser. It proves the display contract with real `sema-retry` source data while
the production design preserves correctness by using `sema-reader`.
