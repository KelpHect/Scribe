# AGENTS.md

## Scope
- Make small, focused changes for Scribe, a Wails desktop app that manages ESO addons from ESOUI/MMOUI.
- Do not commit, tag, push, publish releases, dispatch workflows, or perform release automation; RusticTools/user owns git operations.
- Do not add alternate addon sources, account/cloud sync, telemetry, plugin APIs, signing/notarization, or broad rewrites unless explicitly requested and backed by an accepted plan.
- Do not delete, move, or bulk-modify user addon folders except for the explicitly named install/update/uninstall action.
- Treat `frontend/wailsjs/`, `frontend/dist/`, `build/bin/`, `node_modules/`, `frontend/tsconfig.tsbuildinfo`, build reports, and packaged binaries as generated; do not hand-edit or commit them.

## Project
- Default branch and CI/release workflows target `main`.
- Prefer maintainer-style diffs: smallest correct change, no drive-by refactors, no comment spam, comments only for constraints/trade-offs.
- Final responses should lead with the user-visible change, then checks run, then risks/follow-up.
- If release/packaging/version behavior changes, call it out clearly; release tags are derived from `frontend/package.json` as `vX.Y.Z`.

## Current direction
- The active product is the existing Wails/Svelte/Go app. Do not revive the deleted Avalonia/native rewrite unless the user asks for a new accepted plan.
- The current quality goal is a lighter, smoother, more stable desktop app: faster startup, less UI jank, safer installs/updates, clearer recovery, and fewer moving parts.
- Svelte 5 remains the default frontend until measurements prove another choice is worth the migration cost.
- SolidJS, Electron, Tauri, custom WebKit shells, or other desktop/frontend stacks are allowed only as measured spikes or documented architecture evaluations, not speculative rewrites.
- Electron is not assumed lighter by default because it bundles its own runtime; consider it only if its stability, tooling, or user experience wins outweigh package size and memory cost.
- Prefer improving the current data flow, bridge traffic, cache behavior, install pipeline, and UI rendering before replacing the shell or framework.

## Completion gates
- For Go/app changes, run `wails build` and `go test ./...` from the repo root.
- For frontend changes, also run `npm --prefix frontend run check` and `npm --prefix frontend run test`; run `npm --prefix frontend run build` when touching bundling, styling, assets, Wails bindings, or route/component loading.
- Use `npm --prefix frontend install` to restore frontend deps; do not use another package manager.
- Use `npm --prefix frontend run lint:check` for non-mutating lint verification when lint rules/config or broad frontend code shape changes.
- Avoid `npm --prefix frontend run lint` unless intentionally applying eslint autofixes, because the script runs `eslint . --fix`.
- Clean-checkout caveat: root `go test ./...` fails if `frontend/dist` is absent because `main.go` embeds `all:frontend/dist`; run Wails/build first.
- Clean-checkout caveat: frontend type checks fail if `frontend/wailsjs` is absent/stale; regenerate via `wails dev`/`wails build`, never by authoring generated bindings.
- `npm --prefix frontend run check` is expected to pass after bindings are regenerated; rerun it before claiming frontend package or type-check changes are clean.
- For performance-sensitive changes, run the relevant benchmark path: `./scripts/benchmarks.sh`, `scripts/profile-backend.sh`, `npm --prefix frontend run bench -- --run`, or a narrower package benchmark when appropriate.
- For docs-only AGENTS/TODO audits, at minimum run `git diff --check`; do not run heavyweight app builds unless code/config behavior changed.

## Priorities
1. Keep install, update, dependency install, and uninstall operations safe for the user's AddOns directory.
2. Preserve fast desktop startup and low memory use; diagnostics target <1s frontend-ready and <=150 MB Go runtime/system memory.
3. Favor cached/offline-friendly ESOUI catalog behavior while refreshing stale remote data in the background.
4. Keep UI state stable across navigation; avoid unnecessary TanStack Query invalidation/refetch loops.
5. Reduce jank with measured, local fixes before replacing frameworks or adding abstractions.
6. Prefer smallest correct changes over broad refactors.

## Stack
- Go 1.26.3, Wails v2.12, Node.js 24/npm 11, Svelte 5 runes, TypeScript 6, Vite 8, Tailwind CSS v4.
- SQLite uses GORM with `glebarez/sqlite`; app settings/cache live under the user config dir in `Scribe/esoui_cache.db`.
- Echo is only an indirect Wails dependency; the app has no application HTTP server except opt-in pprof.
- Wails binds methods on `App` in `app.go`; frontend calls them through thin service wrappers and dynamic imports.
- This is not SvelteKit: no SSR, file router, server endpoints, load functions, or SvelteKit APIs.
- Linux Wails builds require `webkit2_41` tags plus GTK/WebKit system dependencies. Debian/Ubuntu use `build-essential pkg-config npm libgtk-3-dev libwebkit2gtk-4.1-dev`; Fedora uses `gcc-c++ pkgconf-pkg-config npm gtk3-devel webkit2gtk4.1-devel`.

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
- Do not introduce framework-agnostic abstraction layers, service locators, event buses, or plugin-style extension points unless they remove proven duplication or fix a measured problem.
- Keep expensive transforms in pure helpers where they can be tested and benchmarked; avoid burying catalog filtering, matching, or install planning inside component markup.
- Do not move logic into the frontend merely to avoid Go changes when the operation belongs next to filesystem, SQLite, archive, or ESOUI code.
- Missing dependency installs resolve a dependency folder to the latest canonical ESOUI catalog entry; manifest version constraints are shown for context and must not pin downloads to an older dependency release.

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
- Keep large lists virtualized with stable item dimensions, fixed image boxes, and bounded overscan.
- Remote addon/list artwork should use fixed-size boxes, lazy loading, async decoding, and failure fallbacks so image fetches do not resize virtual rows or leave broken image chrome.
- Addon detail data and screenshot rails are intentionally bounded; use the `addon-detail-cache` helpers instead of open-ended TanStack detail queries or unbounded screenshot rendering.
- Coalesce high-frequency bridge events before writing to reactive stores; state transitions can be immediate, byte/progress updates should not force avoidable re-render loops.
- Download progress store updates intentionally apply state transitions immediately but batch same-state byte/file progress through animation-frame flushing; preserve this split when changing task-center behavior.
- Backend download/extraction progress is intentionally throttled separately from task state changes; queued/planning/downloading/extracting/complete/failed/cancelled transitions should stay immediate while byte/file counters use the adaptive progress interval.
- Install preflight UI should use the shared helpers in `frontend/src/lib/install/preflight.ts` so task center, update rows, and addon detail dialogs explain add/replace folders and rollback behavior consistently.
- Startup cleanup may remove only stale Scribe-owned `.scribe-staging-*` and `.scribe-backup-*` directories under the configured AddOns folder; never broaden this to ordinary addon folders or arbitrary temp paths.
- Missing dependency UI must keep required and optional groups separate, show unresolved dependencies with the backend plan reason, and install only deduped latest-canonical ESOUI matches.
- Keep search/filter/sort work indexed or memoized for large catalogs; do not repeatedly lowercase, parse versions, score search, or sort compatibility data inside hot render paths.
- Keep Find More catalog indexing/filtering in the tested pure helpers under `frontend/src/lib/perf`; route components should pass state into those helpers instead of rebuilding search/sort logic inline.
- Prefer native desktop-feeling utility UI over marketing layouts, decorative effects, or large animation-heavy surfaces.

## Performance rules
- Measure before optimizing and record the baseline in the task notes when the change is performance-motivated.
- Treat startup scan, remote catalog load, Find More filtering/sorting, virtual list scrolling, image/detail loading, download progress updates, and install extraction as the main hot paths.
- Prefer cache reuse, incremental work, batching, throttling, and pure helper optimization over broad rewrites.
- Avoid adding dependencies for small utilities when a small local helper is clearer and cheaper.
- Remove unused dependencies only after verifying usage and lockfile effects with npm; do not churn packages for aesthetics.
- A framework or shell migration must have a spike branch/plan with measured bundle size, startup time, memory, scroll/search latency, install-progress responsiveness, packaging impact, and regression risk.

## Security/safety rules
- Never allow archive extraction or uninstall paths to escape the configured AddOns directory; preserve zip-slip checks and folder-name validation.
- Validate destructive operations by folder name only; reject empty, dot, slash/backslash, traversal, and absolute-path semantics.
- Preserve cancellation/shutdown behavior for downloads and background refreshes; avoid goroutine leaks around Wails shutdown.
- Queued-download cancellation has regression coverage; keep `Cancel`, `CancelAll`, and shutdown ownership of `sync.WaitGroup` `Done()` calls with the worker goroutines.
- Do not log or expose arbitrary local paths beyond intentional UI for configured addon path/open-folder behavior.
- Do not require secrets, certificates, GitHub tokens, maintainer credentials, or a user's real AddOns directory for tests.

## Tests and fixtures
- Existing tests cover scanner parsing/path detection, ESOUI cache/client/install/download behavior, settings persistence, app-level safety helpers, and frontend store/service smoke flows; add focused tests near the package being changed.
- Scanner invariant: canonical manifests named after the folder (`Folder.addon`/`Folder.txt`) must win over stub files.
- Add/keep regression tests for parser dependency fields (`DependsOn`, `PCDependsOn`, `OptionalDependsOn`), color-code stripping, version matching, cache/settings DB round trips, extraction boundaries, and uninstall validation when touching those areas.
- Use temp dirs/temp SQLite files for filesystem/database tests; never point tests at a real ESO AddOns directory or user config DB.
- Prefer ESOUI fixture data or mocked clients for remote behavior; stop and ask if expected MMOUI behavior cannot be inferred safely.

## Release/deployment rules
- CI builds Windows, Linux, and macOS on `main`/PR, then runs frontend type checks and `go test ./...`.
- Release workflow validates `RELEASE_TAG` against `frontend/package.json`, then publishes mandatory assets: Windows portable, Linux binary, and macOS universal zip. The Windows NSIS installer is optional.
- Tag-release workflow is manual-only; it reads `frontend/package.json` version, validates `X.Y.Z`, creates `vX.Y.Z`, and dispatches release; do not trigger it.
- Local release scripts inject version/commit/date ldflags and may use UPX if installed; do not assume UPX exists.
- Windows builds are unsigned, Linux CI builds use UPX, and macOS builds are ad-hoc signed/not notarized; do not claim stronger distribution guarantees.

## Lessons learned
- `frontend/wailsjs` absence causes frontend type/check failures; regenerate with Wails instead of committing generated files.
- `frontend/dist` absence breaks root Go tests because embedded assets are required.
- Cache schema/version changes must intentionally invalidate or migrate SQLite cache.
- Settings AddOns path changes can diverge if routed through `SetAddonPath` without `SaveSettings`; persist path-changing UI flows.
- The Auto Update setting is persisted but not implemented as a worker; do not describe it as active behavior unless implementing a safe opt-in flow.
- `SCRIBE_PPROF=1` starts the local pprof server; `SCRIBEEGO_PPROF=1` remains a legacy alias.
- `OpenPath` is constrained to the configured AddOns directory or descendants after symlink resolution; justify any expansion of frontend-provided paths.

## Blockers: stop and ask
- A request would delete, move, or bulk-modify user addon directories beyond the named install/update/uninstall action.
- A feature requires a new remote addon source, account/cloud sync, telemetry, signing/notarization, release publishing, or maintainer credentials.
- You need secrets, certificates, GitHub tokens, workflow dispatch/publish rights, or access to a user's real AddOns directory.
- Required checks fail for reasons unrelated to your change and fixing them is outside scope.
- You cannot reproduce or safely infer expected ESOUI/MMOUI API behavior without network access or fixture data.
