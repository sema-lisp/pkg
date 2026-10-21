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

- The package's API reference link sits in its metadata sidebar.
- API and source views share the package header, breadcrumbs, format actions,
  and file/export navigation. The source view no longer has a large back link.
- `package.sema` is the file node above its exports. The index has consistent
  row spacing. With JavaScript, the tree starts collapsed at 800px and below,
  with a bounded, scrollable list when expanded. Without JavaScript, the native
  disclosure remains open by default.
- Source excerpts remove their enclosing module's indentation for display.
  Full source, line numbers, checksums, and stored data are unchanged.
- Format actions are `Markdown` and `Print`. The Print link's title explains
  that the standalone page can also be saved offline.
- Source metadata sits beside the filename. The 12-character SHA-256 prefix is
  a display abbreviation only; clicking copies all 64 characters. File details
  exposes the full value even without JavaScript or clipboard access.

The checksum control follows Primer's [truncation](https://primer.style/product/components/truncate/)
and [copy](https://primer.style/product/getting-started/rails/components/clipboard_copy/)
patterns: compact presentation, access to the complete value, explicit copy
action, and success feedback. Twelve characters is a local display choice,
not a checksum verification rule or a claim of uniqueness.
