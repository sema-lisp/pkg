// Trimmed @sema-lang/ui bundle — only the components the registry uses.
// Built to static/sema-ui.js (committed); rebuild with `npm run build:ui`.
//
// We import the individual component source modules (not the `@sema-lang/ui`
// barrel) so Rollup never pulls in the Shiki-backed components (sema-code,
// sema-editor, sema-markdown, …). Each module's `@customElement` decorator
// registers its element as a side-effect, so the bare imports are enough.
import '@sema-lang/ui/lib/sema-tabs.js'
import '@sema-lang/ui/lib/sema-input.js'
import '@sema-lang/ui/lib/sema-textarea.js'
import '@sema-lang/ui/lib/sema-select.js'
import '@sema-lang/ui/lib/sema-field.js'
import '@sema-lang/ui/lib/sema-button.js'
import '@sema-lang/ui/lib/sema-badge.js'
