# Public page layout audit

Audited on 2026-10-21 against `08eef65` and the API reference refinements.
Widths below assume the default 16px root font size. The page shell includes
its horizontal padding because the shared stylesheet uses `border-box`.

| Route / view | Previous maximum | Current maximum | Inner content limit |
| --- | --- | --- | --- |
| `/` | 64rem / 1,024px | Shared 64rem | Home search: 28rem / 448px |
| `/search` | 64rem / 1,024px | Shared 64rem | Header search: 22rem / 352px |
| `/packages/{name}` | 64rem / 1,024px | Shared 64rem | README plus 240px metadata sidebar |
| `/packages/{name}/{version}` | 64rem / 1,024px | Shared 64rem | Same package layout |
| `/packages/{name}/{version}/api` | **1,180px** | **Shared 64rem** | 14.5rem file/export index; flexible article column; prose capped at 72ch |
| `/packages/{name}/{version}/source/{path}` | **1,180px** | **Shared 64rem** | Same file/export index; source scrolls horizontally within its column |
| `/login` | 64rem / 1,024px | Shared 64rem | 22rem / 352px form |
| Package/version not found and HTML error page | 64rem / 1,024px | Shared 64rem | 560px message block, including its padding |
| `/packages/{name}/{version}/api/print` | 52rem / 832px content | Unchanged | Standalone document: 880px including screen padding; paper uses 18mm page margins |

The API and source pages were the inconsistent public page shells. They now
inherit `--page-max-width: 64rem` through `.page-content`, like the rest of the
registry. At desktop width this leaves 960px after the shell's 32px side padding.
At 640px and below, the shell uses 16px side padding. The narrow login and error
blocks are content limits within that same shell. Print remains an independent
reading format, without site navigation or external assets.

The shared site header and footer span the viewport. Package, API, source, and
search headers use the same 22rem search control; a previous CSS cascade allowed
the extracted package search partial to grow to the generic 32rem search limit.
The named `.search-bar.package-search` rule now owns that limit, including the
full-width mobile row. The search results retain their query value.

Authenticated `/account` and `/link` also use the 64rem shell. Account field
groups are capped at 384px and the repository-link form at 32rem / 512px.
`/admin` is a separate full-width application layout, outside the public-page
audit. JSON APIs, health checks, OAuth redirects, feeds, sitemaps, and `.md`
responses have no HTML layout width.

## Related reference changes

- The package's API reference link sits directly below Repository in its metadata sidebar.
- API and source views share the package header, breadcrumbs, format actions,
  and export navigation. Every export link goes to its API entry from either view.
  The source view no longer has a large back link.
- The source-file link is below the exports. Search uses a placeholder and an
  accessible name, with no visible label or count. Matching counts are announced
  to screen readers. UI ellipses use three periods. The index has consistent
  row spacing. With JavaScript, the index starts collapsed at 800px and below,
  with a bounded, scrollable list when expanded. Without JavaScript, the native
  disclosure remains open by default.
- Source excerpts remove their enclosing module's indentation for display.
  Full source, line numbers, checksums, and stored data are unchanged.
- Format actions are `Markdown` and `Print`. The Print link's title explains
  that the standalone page can also be saved offline.
- Source metadata sits beside the filename. The 12-character SHA-256 prefix is
  shown without a label or ellipsis, beside a copy icon. Clicking copies all 64
  characters. The tooltip and accessible name identify SHA-256; File details
  exposes the full value even without JavaScript or clipboard access.
- All four disclosure controls use the main website's rounded SVG chevron
  (`CustomPageLayout.vue`), with right/down orientation for closed/open states.
  They retain native details/summary keyboard and no-JavaScript behavior.
- README code blocks and tables, API excerpts, source, and the export index use
  thin scrollbars with the registry's muted text and dark code colors. Older
  WebKit gets a 6px fallback. README blocks are ordinary HTML inside the tab
  panel, so this styling does not require a UI package change.

The checksum control follows Primer's [truncation](https://primer.style/product/components/truncate/)
and [copy](https://primer.style/product/getting-started/rails/components/clipboard_copy/)
patterns: compact presentation, access to the complete value, explicit copy
action, and success feedback. Twelve characters is a local display choice,
not a checksum verification rule or a claim of uniqueness.

## Package sidebar order

Compared current package pages on 2026-10-21:

| Registry | Observed order |
| --- | --- |
| [npm](https://www.npmjs.com/package/axios) | Install, Repository, Homepage, downloads, version/license/publish metadata, collaborators, analysis/report links |
| [Packagist](https://packagist.org/packages/symfony/console) | Package info (repository, homepage), security, funding, statistics |
| [NuGet](https://www.nuget.org/packages/Newtonsoft.Json) | Downloads, About (updated date, project website, repository, license and package actions), owners, tags |
| [crates.io](https://crates.io/crates/serde) | Metadata, install, homepage, documentation, repository, owners, categories; download statistics below |

There is no universal order. These examples group project links together and
place owners after those links where an owner group is present. Pkg uses
Repository, Documentation (API reference), Downloads, Sema Version,
Dependencies, Versions, Owners, then reporting. Optional fields are omitted
when unavailable; GitHub-linked packages retain their Source/provenance block
above these groups. The install command stays above the README.

The package grid uses a zero-minimum flexible README column. This keeps long
code lines inside their scroll area instead of pushing the sidebar outside the
viewport. Repository URLs can wrap within their column.

## Nested package preview

`tests/fixtures/nested-api-demo` is a runnable local fixture, not a public release.
Its declared entrypoint is `src/api.sema`; it also contains `src/internal/`,
`tests/`, and `examples/`. Two direct functions are documented. The imported
`demo/triple` alias remains unresolved, as the extractor does not follow imports.
The source viewer exposes the entrypoint only, not an archive file browser.

With the preview server running, seed this fixture using
`node scripts/preview-api-docs.mjs http://localhost:4177 --nested-fixture`.
Run its five assertions with `sema tests/api.sema` from the fixture directory.
