# Frontend

This is the Svelte frontend for Scribe. No SvelteKit, no SSR, no router magic. Wails owns the shell and the frontend just behaves like a desktop UI.

## Commands

```bash
npm install
npm run dev
npm run build
```

## Gotchas

- keep `vitePreprocess()` in default mode. turning on `script: true` breaks template-only imports in Svelte 5
- query caching is intentionally sticky b/c desktop apps regain focus all the time and the Go side already serves hot data
