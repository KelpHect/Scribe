# Dependency Audit

Last updated: 2026-05-18

## Summary

No declared runtime dependency was removed. The current dependencies are used by live code, build config, tests, or the Wails/SQLite/TOML runtime path.

`npm audit fix` updated the lockfile-only `brace-expansion` dev dependency from `5.0.5` to `5.0.6`, clearing the moderate npm advisory without changing `frontend/package.json`.

## Frontend

Verified with:

```bash
rm -rf frontend/node_modules
npm --prefix frontend ci
npm --prefix frontend ls --depth=0
npm --prefix frontend audit --audit-level=moderate
npm --prefix frontend run build
```

Declared frontend dependencies are used as follows:

- `@tanstack/svelte-form`: Settings form state.
- `@tanstack/svelte-query`: route/detail/catalog query caching.
- `@tanstack/svelte-virtual`: Installed and Find More list virtualization.
- `lucide-svelte`: app icons. It is deprecated upstream in favor of `@lucide/svelte`, but a package rename should be handled as a focused compatibility task, not hidden inside a dependency cleanup.
- `svelte`: app framework.
- `svelte-sonner`: toast/task notifications.
- `valibot`: Settings validation schemas.

The clean `npm ci` tree still reports `@emnapi/*`, `@napi-rs/wasm-runtime`, `@tybys/wasm-util`, and `tslib` as extraneous. They come from optional/bundled WASM bindings in the Rolldown/Tailwind toolchain and are not committed app dependencies.

Build comparison after the lockfile-only advisory fix:

- `npm --prefix frontend run build` passed.
- Bundle budget warnings remained the existing non-fatal warnings for `index`, `route-installed`, and `route-settings`; no new dependency was added to the frontend package manifest.

## Go

Verified with:

```bash
go list -m all
go mod tidy
go test ./...
```

Direct Go dependencies are used as follows:

- `github.com/glebarez/sqlite`: GORM SQLite driver.
- `github.com/google/uuid`: search preset IDs and local app identifiers.
- `github.com/pelletier/go-toml/v2`: atomic `settings.toml` persistence.
- `github.com/wailsapp/wails/v2`: desktop shell, app bindings, runtime helpers, and build tooling.
- `gorm.io/gorm`: cache/settings migration, search presets, scanner cache, and install records.

Indirect Go dependencies remain owned by Wails, GORM, SQLite, and platform/runtime packages.
