# Dependency Audit

Last updated: 2026-05-19

## Summary

The current runtime dependencies are used by live code, build config, tests, or the Wails/SQLite/TOML runtime path.

`npm audit fix` updated the lockfile-only `brace-expansion` dev dependency from `5.0.5` to `5.0.6`, clearing the moderate npm advisory. A later cleanup added `@tanstack/query-core` as an explicit direct dependency because frontend code imports `QueryClient` directly.

The frontend lint/format stack was simplified by removing ESLint, TypeScript ESLint, Svelte ESLint config/plugin, Prettier, and Prettier plugins, then adding Oxlint and Oxfmt. Oxfmt is used for supported TypeScript, JavaScript, and CSS files; `.svelte` component formatting remains governed by focused edits, review, Svelte language checks, and Oxlint diagnostics.

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
- `@tanstack/query-core`: direct `QueryClient` use in shared query/cache helpers.
- `@tanstack/svelte-query`: route/detail/catalog query caching.
- `@tanstack/svelte-virtual`: Installed and Find More list virtualization.
- `lucide-svelte`: app icons. It is deprecated upstream in favor of `@lucide/svelte`, but a package rename should be handled as a focused compatibility task, not hidden inside a dependency cleanup.
- `svelte`: app framework.
- `svelte-sonner`: toast/task notifications.
- `valibot`: Settings form validation in `SettingsPage.svelte`.

The clean `npm ci` tree still reports `@emnapi/*`, `@napi-rs/wasm-runtime`, `@tybys/wasm-util`, and `tslib` as extraneous. They come from optional/bundled WASM bindings in the Rolldown/Tailwind toolchain and are not committed app dependencies.

Current build comparison:

- `npm --prefix frontend run build` passed.
- Bundle budget warnings remain non-fatal and currently cover small overages in `index`, `route-installed`, `route-settings`, and `index.css`.

## Go

Verified with:

```bash
go list -m all
go mod tidy
go test ./...
```

Direct Go dependencies are used as follows:

- `github.com/glebarez/sqlite`: GORM SQLite driver.
- `github.com/google/uuid`: search preset row IDs and local app identifiers.
- `github.com/pelletier/go-toml/v2`: atomic `settings.toml` persistence.
- `github.com/wailsapp/wails/v2`: desktop shell, app bindings, runtime helpers, and build tooling.
- `gorm.io/gorm`: cache/settings migration, search preset rows, scanner cache, and install records.

Indirect Go dependencies remain owned by Wails, GORM, SQLite, and platform/runtime packages.
