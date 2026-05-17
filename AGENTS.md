# AGENTS.md

## Scope
- Make small, focused changes for Scribe, a Wails desktop app that manages ESO addons from ESOUI/MMOUI.
- Do not commit, tag, push, publish releases, dispatch workflows, or perform release automation; RusticTools/user owns git operations.
- Do not add alternate addon sources, account/cloud sync, telemetry, plugin APIs, signing/notarization, or broad rewrites unless explicitly requested.
- Do not delete, move, or bulk-modify user addon folders except for the explicitly named install/update/uninstall action.
- Treat `frontend/wailsjs/`, `frontend/dist/`, `build/bin/`, `node_modules/`, `frontend/tsconfig.tsbuildinfo`, build reports, and packaged binaries as generated; do not hand-edit or commit them.

## Project
- Default branch and CI/release workflows target `main`.
- Prefer maintainer-style diffs: smallest correct change, no drive-by refactors, no comment spam, comments only for constraints/trade-offs.
- Final responses should lead with the user-visible change, then checks run, then risks/follow-up.
- If release/packaging/version behavior changes, call it out clearly; release tags are derived from `frontend/package.json` as `vX.Y.Z`.

## Completion gates
- For Go/app changes, run `wails build` and `go test ./...` from the repo root.
- For frontend changes, also run `npm --prefix frontend run check`; run `npm --prefix frontend run build` when touching bundling, styling, assets, Wails bindings, or route/component loading.
- Use `npm --prefix frontend install` to restore frontend deps; do not use another package manager.
- Avoid `npm --prefix frontend run lint` unless intentionally applying eslint autofixes, because the script runs `eslint . --fix`.
- Clean-checkout caveat: root `go test ./...` fails if `frontend/dist` is absent because `main.go` embeds `all:frontend/dist`; run Wails/build first.
- Clean-checkout caveat: frontend type checks fail if `frontend/wailsjs` is absent/stale; regenerate via `wails dev`/`wails build`, never by authoring generated bindings.
- Current baseline caveat from TODO: `npm --prefix frontend run check` may also expose existing TS issues after bindings are regenerated; do not claim it passes unless rerun successfully in this workspace.
- For docs-only AGENTS/TODO audits, at minimum run `git diff --check`; do not run heavyweight app builds unless code/config behavior changed.

## Priorities
1. Keep install, update, dependency install, and uninstall operations safe for the user's AddOns directory.
2. Preserve fast desktop startup and low memory use; diagnostics target <1s frontend-ready and <=150 MB Go runtime/system memory.
3. Favor cached/offline-friendly ESOUI catalog behavior while refreshing stale remote data in the background.
4. Keep UI state stable across navigation; avoid unnecessary TanStack Query invalidation/refetch loops.
5. Prefer smallest correct changes over broad refactors.

## Stack
- Go 1.23, Wails v2.12, Svelte 5 runes, TypeScript, Vite 8, Tailwind CSS v4.
- SQLite uses GORM with `glebarez/sqlite`; app settings/cache live under the user config dir in `Scribe/esoui_cache.db`.
- Echo is only an indirect Wails dependency; the app has no application HTTP server except opt-in pprof.
- Wails binds methods on `App` in `app.go`; frontend calls them through thin service wrappers and dynamic imports.
- This is not SvelteKit: no SSR, file router, server endpoints, load functions, or SvelteKit APIs.
- Linux Wails builds require `webkit2_41` tags plus GTK/WebKit system dependencies (`libgtk-3-dev`, `libwebkit2gtk-4.1-dev`).

## Documentation paths
- Keep `README.md`, `CONTRIBUTING.md`, and `frontend/README.md` synchronized with setup, checks, generated-file recovery, packaging, and release-process changes.
- Keep `TODO.md` as the durable audit/backlog ledger; update it when audits reveal or close safety, baseline, or workflow gaps.
- Keep `frontend/package.json` version accurate when requested; tag-release workflow creates tags from it.
- Update this file when a new invariant, generated-file rule, required check, blocker, baseline caveat, or regression lesson is discovered.

## Architecture boundaries
- `app.go` is the Wails boundary: expose user-facing operations there and delegate parsing, scanning, downloading, caching, matching, installs, and settings.
- `internal/scanner` owns ESO AddOns path detection and `.txt`/`.addon` manifest parsing.
- `internal/esoui` owns MMOUI/ESOUI API access, remote cache/schema, addon matching, downloads, archive extraction, install MD5 records, and uninstall safety.
- `internal/settings` owns persisted app settings only; do not mix settings persistence into UI or ESOUI client code.
- Frontend service modules in `frontend/src/lib/services` stay thin Wails/runtime wrappers; shared client state belongs in `frontend/src/lib/stores` or TanStack Query helpers under `frontend/src/lib/db`.
- Route components under `frontend/src/routes` compose pages and queries; reusable UI belongs under `frontend/src/lib/components`.
- External URLs must go through Wails runtime helpers from frontend services; do not add ad-hoc browser/process launching for URLs.

## Data and persistence rules
- Preserve the historical config directory name `Scribe`; changing it strands existing `esoui_cache.db` settings/cache.
- Cache TTL is 4 hours and schema-versioned (`cacheSchemaVersion`); cache schema changes must intentionally migrate or invalidate SQLite data.
- Do not store settings in frontend-only state when they must survive restart; persist through `settings.Manager`/`SaveSettings`.
- Search presets and install MD5 records share the app DB; keep migrations compatible with GORM `AutoMigrate`.
- ESOUI MD5 is for download integrity and suppressing update false positives only; never present it as cryptographic security.
- Network behavior is MMOUI bootstrap (`https://api.mmoui.com/v3/globalconfig.json`) plus discovered ESO feeds; tests should not require live ESOUI unless explicitly marked/manual.

## Frontend/UI rules
- Keep `vitePreprocess()` in default mode; enabling `script: true` breaks Svelte 5 template-only imports.
- Preserve dynamic route imports and manual chunks unless intentionally changing startup/bundle behavior; frontend build reports warn on budget violations but do not fail.
- TanStack Query caching is intentionally sticky (`staleTime`, long `gcTime`, no focus refetch); do not add focus-driven refetch loops.
- Keep mounted route state stable across navigation where current UX relies on it.
- `frontend/wailsjs` imports are generated; frontend services should use `callWails`/runtime wrappers instead of duplicating binding logic.
- Global hotkeys/context menus and memory cleanup live in `App.svelte`; avoid page-level listeners that leak or conflict with global behavior.

## Security/safety rules
- Never allow archive extraction or uninstall paths to escape the configured AddOns directory; preserve zip-slip checks and folder-name validation.
- Validate destructive operations by folder name only; reject empty, dot, slash/backslash, traversal, and absolute-path semantics.
- Preserve cancellation/shutdown behavior for downloads and background refreshes; avoid goroutine leaks around Wails shutdown.
- Known P0 hazard: queued-download cancellation currently risks `sync.WaitGroup` underflow/double `Done`; fix with regression tests before relying on broad cancel/shutdown changes.
- Do not log or expose arbitrary local paths beyond intentional UI for configured addon path/open-folder behavior.
- Do not require secrets, certificates, GitHub tokens, maintainer credentials, or a user's real AddOns directory for tests.

## Tests and fixtures
- Existing Go coverage is mostly scanner manifest selection in `internal/scanner/scanner_test.go`; add focused table tests near the package being changed.
- Scanner invariant: canonical manifests named after the folder (`Folder.addon`/`Folder.txt`) must win over stub files.
- Add/keep regression tests for parser dependency fields (`DependsOn`, `PCDependsOn`, `OptionalDependsOn`), color-code stripping, version matching, cache/settings DB round trips, extraction boundaries, and uninstall validation when touching those areas.
- Use temp dirs/temp SQLite files for filesystem/database tests; never point tests at a real ESO AddOns directory or user config DB.
- Prefer ESOUI fixture data or mocked clients for remote behavior; stop and ask if expected MMOUI behavior cannot be inferred safely.

## Release/deployment rules
- CI builds Windows, Linux, and macOS on `main`/PR, then runs `go test ./...`; CI currently does not run frontend type checks.
- Release workflow publishes tag assets: Windows portable/optional NSIS installer, Linux binary, macOS universal zip.
- Tag-release workflow reads `frontend/package.json` version, validates `X.Y.Z`, creates `vX.Y.Z`, and dispatches release; do not trigger it.
- Local release scripts inject version/commit/date ldflags and may use UPX if installed; do not assume UPX exists.
- Windows builds are unsigned, Linux CI builds use UPX, and macOS builds are ad-hoc signed/not notarized; do not claim stronger distribution guarantees.

## Lessons learned
- `frontend/wailsjs` absence causes frontend type/check failures; regenerate with Wails instead of committing generated files.
- `frontend/dist` absence breaks root Go tests because embedded assets are required.
- Cache schema/version changes must intentionally invalidate or migrate SQLite cache.
- Settings AddOns path changes can diverge if routed through `SetAddonPath` without `SaveSettings`; persist path-changing UI flows.
- The Auto Update setting is persisted but not implemented as a worker; do not describe it as active behavior unless implementing a safe opt-in flow.
- `pprof.go` currently uses `SCRIBEEGO_PPROF`; document or rename compatibly before relying on it.
- `OpenPath` is broad OS shell opening; constrain or justify any expansion of frontend-provided paths.

## Blockers: stop and ask
- A request would delete, move, or bulk-modify user addon directories beyond the named install/update/uninstall action.
- A feature requires a new remote addon source, account/cloud sync, telemetry, signing/notarization, release publishing, or maintainer credentials.
- You need secrets, certificates, GitHub tokens, workflow dispatch/publish rights, or access to a user's real AddOns directory.
- Required checks fail for reasons unrelated to your change and fixing them is outside scope.
- You cannot reproduce or safely infer expected ESOUI/MMOUI API behavior without network access or fixture data.
