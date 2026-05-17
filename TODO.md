# Scribe TODO Audit Ledger

Last audit refresh: 2026-05-17

Audit scope inspected: Go app boundary and internal packages, Svelte frontend routes/components/services/stores, tests, README/CONTRIBUTING/frontend README/AGENTS, scripts, Wails config, Go/npm package config, and GitHub workflows. No `docs/`, `plan/`, `roadmap/`, `todo/`, or `backlog/` files were present outside this ledger.

## P0 — Safety, data-loss prevention, and shutdown correctness

P0 covers work that can corrupt/delete user AddOns content, leave downloads/extractions in an unsafe state, or panic/hang on cancellation/shutdown. Scope guardrail: keep fixes narrow; do not bulk-modify user addon folders beyond the named install/update/uninstall action.

- [ ] Fix queued-download cancellation so it cannot underflow `sync.WaitGroup` or panic during shutdown.
  - Evidence: `internal/esoui/download_manager.go` starts one `processNext` goroutine per enqueue, but `Cancel`/`CancelAll` call `dm.wg.Done()` for queued tasks while the goroutine can still later execute its deferred `wg.Done()`.
  - Acceptance criteria: cancelling queued tasks and calling `Shutdown()` after enqueuing more tasks than concurrency is covered by tests and does not panic, race, or hang.
- [ ] Add safety regression tests for archive extraction and uninstall boundaries.
  - Evidence: `internal/esoui/installer.go` has zip-slip and folder-name validation, but no tests exercise malicious `../` zip entries, absolute paths, invalid folder names, or valid nested extraction.
  - Acceptance criteria: tests prove extraction rejects paths escaping the configured AddOns directory; uninstall rejects empty/dot/slash/backslash traversal names; valid addon folder removal still works.
- [ ] Preserve install/update cancellation behavior while extracting large archives.
  - Evidence: `ExtractWithProgress` checks context before each entry, but no test covers cancellation mid-extraction or partial-state expectations.
  - Acceptance criteria: a cancellation test demonstrates a queued/running install transitions to `cancelled` and does not continue extracting after the context is cancelled.

## P1 — User-visible correctness and release/CI confidence

P1 covers behavior that users can see during normal install, update, settings, or release workflows. Scope guardrail: preserve ESOUI/MMOUI as the only addon source and avoid broad UI rewrites.

- [ ] Make AddOns path updates persistent from every Settings path-change action.
  - Evidence: the Settings “Use this path” flow calls `updateAddonPath()` (`SetAddonPath`) directly, while persistence is only in `SaveSettings`; this can diverge in-memory path from saved settings until the user also saves.
  - Acceptance criteria: choosing a detected/browsed path and applying it through any Settings action saves the same path through the settings manager, refreshes installed state, and survives app restart.
- [ ] Resolve the Auto Update preference mismatch.
  - Evidence: `settings.AppSettings.AutoUpdate` is persisted and `SettingsPage.svelte` says “Automatically update addons when updates are available,” but no backend/frontend worker consumes `autoUpdate` to perform updates.
  - Acceptance criteria: either implement a safe opt-in auto-update flow with clear user confirmation/limits, or relabel/disable the setting so the UI no longer claims unimplemented behavior.
- [ ] Restore a clean frontend type-check baseline.
  - Evidence: `npm --prefix frontend run check` on 2026-05-17 failed with missing generated Wails modules, `frontend/src/lib/utils/index.ts` `matchAll` entry typed as `unknown`, and `InstalledPage.svelte` using `addons` before declaration.
  - Acceptance criteria: after regenerating Wails bindings via Wails, `npm --prefix frontend run check` passes without suppressing real type errors.
- [ ] Add frontend type checking to documented/CI verification.
  - Evidence: `frontend/package.json` provides `npm run check`; AGENTS requires it for frontend changes, but `CONTRIBUTING.md` and `.github/workflows/ci.yml` only run `wails build` and `go test ./...`.
  - Acceptance criteria: docs and CI consistently run `npm --prefix frontend run check` after generated Wails bindings are available, without hand-editing `frontend/wailsjs/`.
- [ ] Fix or intentionally document the pprof environment variable name.
  - Evidence: `pprof.go` checks `SCRIBEEGO_PPROF`, which appears inconsistent with the app name and is undocumented in README/CONTRIBUTING.
  - Acceptance criteria: the intended env var is documented or renamed with compatibility if needed.

## P2 — Test coverage and maintainability gaps

P2 covers gaps that increase regression risk but are not immediate user-data hazards.

- [ ] Add parser and matcher unit tests for ESO addon metadata edge cases.
  - Evidence: current Go tests only cover scanner manifest selection in `internal/scanner/scanner_test.go`; parser/matcher behavior is untested despite version comparison, dependency parsing, color-code stripping, and remote directory matching logic.
  - Acceptance criteria: table tests cover `DependsOn`/`OptionalDependsOn`, `PCDependsOn`, color-coded titles, missing titles, version comparisons, sibling/multiple-dir matching, and no-update false positives.
- [ ] Add cache/settings database tests using temporary SQLite files.
  - Evidence: `internal/esoui/cache.go`, `internal/esoui/db.go`, and `internal/settings/settings.go` contain schema versioning, JSON conversion, persisted settings, search presets, and MD5 install records with no direct tests.
  - Acceptance criteria: tests cover schema invalidation, cache round-trip, settings defaults/save/load, invalid theme fallback, and install MD5 save/read.
- [ ] Add frontend smoke tests or component-level checks for core flows.
  - Evidence: no frontend test runner is configured; routes implement install/update/uninstall/settings flows with only `svelte-check` available.
  - Acceptance criteria: a minimal, maintainable frontend test strategy covers store/service behavior or critical components without requiring live ESOUI network access.
- [ ] Document generated-file recovery for clean checkouts.
  - Evidence: AGENTS notes missing `frontend/dist` breaks `go test ./...` and missing `frontend/wailsjs` breaks frontend checks; this audit confirmed `go test ./...` fails at `//go:embed all:frontend/dist` when `frontend/dist` is absent. README/CONTRIBUTING only mention `wails build`/`wails dev` at a high level.
  - Acceptance criteria: contributor docs explain when to run `wails build`/`wails dev` to regenerate `frontend/dist` and Wails bindings, and that generated files should not be hand-edited.

## P3 — Polish and low-risk UX improvements

P3 covers small improvements that can wait until P0/P1 are stable.

- [ ] Improve feedback when remote refresh fails but stale cache exists.
  - Evidence: backend serves cached remote lists and refreshes stale data in the background, while frontend errors are mostly generic (“Failed to load addons from ESOUI”).
  - Acceptance criteria: users can distinguish “showing cached data” from “no data available,” without adding telemetry or a new remote source.
- [ ] Review `OpenPath` usage for least-surprise behavior.
  - Evidence: `app.go` opens any frontend-provided path with OS shell helpers; current UI passes addon folder paths, but the backend method is broad.
  - Acceptance criteria: either constrain calls to configured AddOns descendants where appropriate or document why broader open-folder behavior is required.

## Completed / current-state evidence

- [x] Project agent operating contract was refreshed from code/docs/workflows/config/tests on 2026-05-17.
  - Evidence: `AGENTS.md` now captures scope limits, generated-file rules, Wails/Svelte/SQLite boundaries, completion gates, release constraints, safety invariants, tests/fixtures expectations, and known baseline caveats.
- [x] Wails desktop app boundary exists and delegates domain work to internal packages.
  - Evidence: `app.go` binds methods on `App`; scanner, ESOUI/cache/install/download, and settings logic live under `internal/` packages.
- [x] ESOUI/MMOUI is the only implemented remote addon source.
  - Evidence: `internal/esoui/client.go` bootstraps `https://api.mmoui.com/v3/globalconfig.json`; no alternate remote source implementation was found.
- [x] Installed addon scanning prefers canonical folder-named manifests over stubs.
  - Evidence: `internal/scanner/scanner.go` checks `Folder.addon`/`Folder.txt` first; `TestScanAddonDir_PrefersFolderNameManifest` covers this.
- [x] Basic release automation builds Windows, Linux, and macOS assets from version tags.
  - Evidence: `.github/workflows/release.yml` builds matrix assets; `.github/workflows/tag-release.yml` reads `frontend/package.json` version as `vX.Y.Z`.
- [x] Remote catalog cache is SQLite-backed and schema-versioned.
  - Evidence: `internal/esoui/cache.go` stores addons/categories/meta in `esoui_cache.db` under the historical `Scribe` config dir with `cacheSchemaVersion = "2"`.

## Deferred / future work (not active priorities)

These are intentionally outside current active scope unless the maintainer explicitly requests them: alternate addon sources beyond ESOUI/MMOUI, account/cloud sync, telemetry, plugin APIs, signing/notarization, release publishing, or broad rewrites of the Wails/Svelte architecture.
