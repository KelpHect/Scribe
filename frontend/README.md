# Frontend

This is the Svelte frontend for Scribe. No SvelteKit, no SSR, no router magic. Wails owns the shell and the frontend just behaves like a desktop UI.

## Commands

```bash
npm install
npm run dev
npm run build
npm run check
npm run test
```

## Gotchas

- keep `vitePreprocess()` in default mode. turning on `script: true` breaks template-only imports in Svelte 5
- query caching is intentionally sticky b/c desktop apps regain focus all the time and the Go side already serves hot data
- Vitest smoke tests cover store/service flows with mocked Wails wrappers; do not call live ESOUI from frontend tests
- `wails dev` or `wails build` regenerates `frontend/wailsjs/`; never author generated bindings by hand
- `wails build` regenerates `frontend/dist/`, which root Go tests embed
