import { defineConfig } from 'vite'
import { fileURLToPath } from 'node:url'

// Bundles web/ui-entry.js (the components we use) into static/sema-ui.js + .css.
// Not part of the Rust/Docker build — the outputs are committed; run manually
// with `npm run build:ui` when the component set or @sema-lang/ui changes.
//
// The alias resolves `@sema-lang/ui/lib/*` straight to the package's source
// modules. The package's exports map only exposes the `.` barrel (which drags
// in every component, incl. the Shiki-backed ones), so we deep-import the few
// components we actually use to keep the bundle small.
const libDir = fileURLToPath(new URL('./node_modules/@sema-lang/ui/src/lib', import.meta.url))

export default defineConfig({
  resolve: {
    alias: [{ find: /^@sema-lang\/ui\/lib\/(.*)$/, replacement: `${libDir}/$1` }],
  },
  // Lit components use TypeScript experimental decorators on class fields. esbuild
  // defaults to standard decorators, which crash at runtime ("Unsupported
  // decorator location: field"), so mirror the library's own tsconfig here.
  esbuild: {
    tsconfigRaw: {
      compilerOptions: { experimentalDecorators: true, useDefineForClassFields: false },
    },
  },
  build: {
    outDir: 'static',
    emptyOutDir: false,
    cssCodeSplit: false,
    minify: 'esbuild',
    lib: { entry: 'web/ui-entry.js', formats: ['es'], fileName: () => 'sema-ui.js' },
    rollupOptions: { output: { assetFileNames: 'sema-ui.[ext]' } },
  },
})
