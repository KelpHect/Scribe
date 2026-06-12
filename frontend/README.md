# Frontend

This is the Svelte frontend for Scribe. No SvelteKit, no SSR, no router magic. Wails owns the shell and the frontend just behaves like a desktop UI.

## Commands

From the repo root:

```bash
npm --prefix frontend install
npm --prefix frontend run dev
npm --prefix frontend run build
npm --prefix frontend run check
npm --prefix frontend run test
npm --prefix frontend run lint:check
npm --prefix frontend run format:check
```

`npm --prefix frontend run lint` applies Oxlint fixes. Use `lint:check` for non-mutating verification. `format:check` uses Oxfmt for supported TypeScript, JavaScript, and CSS files; Svelte component formatting is still kept through focused edits and `svelte-check`.

## Gotchas

- keep `vitePreprocess()` in default mode. turning on `script: true` breaks template-only imports in Svelte 5
- query caching is intentionally sticky b/c desktop apps regain focus all the time and the Go side already serves hot data
- Vitest smoke tests cover store/service flows with mocked Wails wrappers; do not call live ESOUI from frontend tests
- `wails3 task common:generate:bindings` or `wails3 build` regenerates `frontend/bindings/`; never author generated bindings by hand
- `wails3 build` regenerates `frontend/dist/`, which root Go tests embed
