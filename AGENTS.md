# AGENTS.md

## Scope
- Make small, focused changes for Scribe, a Wails desktop app that manages ESO addons from ESOUI/MMOUI.
- Do not commit, tag, push, publish releases, or run workflow-dispatch/release automation; RusticTools/user owns git operations.
- Do not add alternate addon sources, account/cloud sync, telemetry, plugin APIs, signing/notarization, or broad rewrites unless explicitly requested.
- Treat `frontend/wailsjs/`, `frontend/dist/`, `build/bin/`, `node_modules/`, `frontend/tsconfig.tsbuildinfo`, and generated build artifacts as generated; do not hand-edit them.

## Project
- Default branch and CI/release workflows target `main`.
- Keep final responses practical: user-visible change first, checks run, then risks/follow-up.
- If release/packaging behavior changes, call it out clearly; version tags are derived from `frontend/package.json` as `vX.Y.Z`.
- Prefer maintainer-style diffs: small, direct, no drive-by refactors or comment spam.

## Completion gates
- For code changes, run `wails build` and `go test ./...` from the repo root.
- For frontend changes, also run `npm --prefix frontend run check`; run `npm --prefix frontend run build` when touching bundling, styling, assets, Wails bindings, or route/component loading.
- Use `npm --prefix frontend install` to restore frontend deps; do not use a different package manager.
- Avoid `npm --prefix frontend run lint` unless intentionally applying eslint autofixes, because the script runs `eslint . --fix`.
- If `frontend/wailsjs/` bindings are missing or stale, regenerate via Wails (`wails dev`/`wails build`), never by authoring generated files.
- Current clean-checkout caveat: without `frontend/dist`, root `go test ./...` fails at `//go:embed all:frontend/dist`; without `frontend/wailsjs`, frontend type checks fail. Do not claim these pass unless rerun successfully in the current workspace.

## Priorities
1. Keep install, update, dependency install, and uninstall operations safe for the user's AddOns directory.
2. Preserve fast desktop startup and low memory use; diagnostics currently target <1s frontend-ready and <=150 MB Go runtime/system memory.
3. Favor cached/offline-friendly ESOUI catalog behavior while refreshing stale remote data in the background.
4. Keep UI state stable across navigation; avoid unnecessary TanStack Query invalidation/refetch loops.
5. Prefer smallest correct changes over broad refactors.

## Stack
- Go 1.23, Wails v2.12, Svelte 5 runes, TypeScript, Vite 8, Tailwind CSS v4.
- SQLite is accessed through GORM with `glebarez/sqlite`; app settings/cache live under the user config dir in `Scribe/esoui_cache.db`.
- Echo appears only as an indirect Wails dependency; this app has no application HTTP server except opt-in pprof.
- Wails binds methods on `App` in `app.go`; frontend calls them through thin service wrappers and dynamic imports.
- This is not SvelteKit: no SSR, file router, server endpoints, or SvelteKit APIs.
- Linux Wails builds require `webkit2_41` tags plus GTK/WebKit system dependencies.

## Documentation paths
- Keep `README.md`, `CONTRIBUTING.md`, and `frontend/README.md` synchronized with setup, check, packaging, and release-process changes.
- Keep `frontend/package.json` version accurate when requested; tag-release workflow creates tags from it.
- Update this file when a new invariant, generated-file rule, required check, blocker, or regression lesson is discovered.

## Architecture boundaries
- `app.go` is the Wails boundary: expose user-facing operations there and delegate parsing, scanning, downloading, caching, matching, installs, and settings.
- `internal/scanner` owns ESO AddOns path detection and `.txt`/`.addon` manifest parsing.
- `internal/esoui` owns MMOUI/ESOUI API access, remote cache/schema, addon matching, downloads, extraction, install records, and uninstall safety.
- `internal/settings` owns persisted app settings only; do not mix settings persistence into UI or ESOUI client code.
- Frontend service modules in `frontend/src/lib/services` stay thin Wails/runtime wrappers; shared state belongs in `frontend/src/lib/stores` or TanStack Query helpers under `frontend/src/lib/db`.
- Route components under `frontend/src/routes` compose pages and queries; reusable UI belongs under `frontend/src/lib/components`.

## Security/safety rules
- Never allow archive extraction or uninstall paths to escape the configured AddOns directory; preserve zip-slip checks and folder-name validation.
- Do not delete, move, or bulk-modify user addon folders except for the explicitly named install/update/uninstall action.
- Use ESOUI MD5 only for download integrity/update false-positive checks; do not present it as cryptographic security.
- Do not log or expose arbitrary local paths beyond intentional UI for the configured addon path/open-folder behavior.
- Preserve cancellation/shutdown behavior for downloads and background refreshes; avoid goroutine leaks around Wails shutdown.
- Open external URLs through Wails runtime helpers (`BrowserOpenURL`) from frontend services; do not add ad-hoc browser/process launching for URLs.

## Lessons learned
- Canonical manifests named after the folder (`Folder.addon`/`Folder.txt`) must win over stub files; scanner tests cover this.
- `vitePreprocess()` must stay in default mode; enabling `script: true` breaks Svelte 5 template-only imports.
- TanStack Query caching is intentionally sticky because desktop apps regain focus often and Go already serves hot data.
- Cache schema/version changes must intentionally invalidate or migrate SQLite cache; keep the historical app config dir name `Scribe` for existing users.
- `frontend/wailsjs` absence causes frontend type/check failures; regenerate bindings with Wails instead of committing generated files.
- Frontend chunk budgets are reported by `frontend/vite-plugin-build-report.js`; size regressions should be intentional even though the plugin warns rather than fails.

## Blockers: stop and ask
- A request would delete, move, or bulk-modify user addon directories beyond the named install/update/uninstall action.
- A feature requires a new remote addon source, account/cloud sync, telemetry, signing/notarization, release publishing, or maintainer credentials.
- You need secrets, certificates, GitHub tokens, or access to a user's real AddOns directory to proceed safely.
- Required checks fail for reasons unrelated to your change and the fix is outside scope.
- You cannot reproduce or safely infer expected ESOUI/MMOUI API behavior without network access or fixture data.
