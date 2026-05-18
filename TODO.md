# Scribe TODO Audit Ledger

Last audit refresh: 2026-05-17

## Audit scope inspected

- Code: `app.go`, `main.go`, `pprof.go`, `internal/addon`, `internal/scanner`, `internal/esoui`, `internal/settings`, and the Svelte frontend under `frontend/src` including routes, components, stores, services, query helpers, theme/runtime/diagnostics flows, and utilities.
- Tests: `internal/scanner/scanner_test.go`; confirmed no other Go tests and no configured frontend test runner beyond `svelte-check`.
- Docs/plans: `AGENTS.md`, `README.md`, `CONTRIBUTING.md`, `frontend/README.md`, and the prior `TODO.md`. No `docs/`, `plan/`, `plans/`, `roadmap/`, `roadmaps/`, or `backlog/` directories/files were present outside this ledger.
- Scripts/config: `go.mod`, `go.sum`, `wails.json`, `frontend/package.json`, `frontend/package-lock.json`, `frontend/vite.config.ts`, `frontend/svelte.config.js`, `frontend/eslint.config.js`, `scripts/build-release.sh`, `scripts/build-release.ps1`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`, and `.github/workflows/tag-release.yml`.
- Generated/runtime surfaces: checked `frontend/wailsjs/` and `frontend/dist/` presence; both are absent in this workspace and are treated as generated. Also inspected build icons/manifests and Wails embed behavior.
- Verification run during audit: `git diff --check` passed; `go test ./...` failed because `frontend/dist` is absent; `npm --prefix frontend run check` failed with missing Wails bindings plus real TypeScript errors.

## P0 — Safety, data-loss prevention, and shutdown correctness

Purpose: prevent corruption, deletion outside the configured AddOns directory, panics, hangs, and unsafe destructive operations. Risk level: critical because failures can damage user addon folders or crash during install/update/cancel. Scope guardrail: keep fixes narrow; do not bulk-modify addon folders beyond the explicitly named install/update/uninstall action.

### Safety/data-loss

- [x] Fix queued-download cancellation so it cannot underflow `sync.WaitGroup` or panic during shutdown.
  - Completed: `Cancel` and `CancelAll` no longer decrement the goroutine `WaitGroup` for queued tasks; the queued `processNext` goroutines remain the only owners of their deferred `Done()` calls.
  - Verification: `internal/esoui/download_manager_test.go` covers cancelling queued tasks after enqueuing more tasks than concurrency, cancelling all queued tasks, and `Shutdown()` with queued tasks; package race tests pass.
- [x] Add regression tests for archive extraction boundaries.
  - Completed: `ExtractWithProgress` now rejects parent traversal, absolute slash paths, Windows drive-style paths, backslash separator variants, and destination-prefix sibling escapes before extraction.
  - Verification: `internal/esoui/installer_test.go` uses temp dirs and generated zip files to prove escaping entries fail and valid nested addon files extract only under the configured destination.
- [x] Add regression tests for uninstall folder-name validation.
  - Completed: `RemoveAddonFolder` is covered for empty, `.`, `..`, slash/backslash, traversal, absolute-looking names, missing folders, and valid folder removal.
  - Verification: `internal/esoui/installer_test.go` uses temp AddOns directories to prove invalid and missing folder names leave sibling/outside directories intact, while a valid named addon folder is removed.
- [x] Add explicit confirmation for list/context/bulk uninstall flows or otherwise require a deliberate destructive step.
  - Completed: `InstalledPage.svelte` now routes row, context-menu, and bulk uninstall actions through one confirmation dialog before calling backend uninstall mutations.
  - Verification: the dialog names the affected addon or lists selected addon folders, and backend folder-name validation remains unchanged.

### Shutdown/cancellation

- [x] Preserve install/update cancellation behavior while extracting large archives.
  - Completed: extraction and download-manager tests now cancel after the first extracted file and prove later files are not written.
  - Verification: `internal/esoui/installer_test.go` asserts `context.Canceled` from `ExtractWithProgress`; `internal/esoui/download_manager_test.go` cancels a running extraction and observes the task state become `cancelled`.

## P1 — User-visible correctness, baseline health, and release confidence

Purpose: fix gaps users or contributors hit in normal settings, install, update, docs, and CI flows. Risk level: high because these issues make visible claims false or break a clean verification baseline. Scope guardrail: preserve ESOUI/MMOUI as the only addon source and avoid broad UI rewrites.

### Correctness/persistence

- [x] Make AddOns path updates persistent from every Settings path-change action.
  - Completed: `App.SetAddonPath` persists successful path changes, and Settings detected/browse actions save the same path immediately before refreshing installed state.
  - Verification: `npm --prefix frontend run check`, `npm --prefix frontend run build`, `go test ./...`, and Linux Wails build pass.
- [x] Resolve the Auto Update preference mismatch.
  - Completed: the Settings UI now marks Auto Update as unavailable/manual-review only, and backend settings keep the inert preference false until a safe worker exists.
  - Verification: `npm --prefix frontend run check`, `npm --prefix frontend run build`, `go test ./...`, and Linux Wails build pass.
- [x] Validate Settings form inputs before persistence.
  - Completed: frontend Settings validation rejects invalid AddOns paths and negative/non-finite memory thresholds, while backend settings persistence rejects invalid non-empty paths and normalizes inert auto-update, negative memory, and unsupported themes.
  - Verification: `internal/settings/settings_test.go` covers invalid path rejection plus memory/theme/auto-update normalization; frontend and app checks pass.

### Baseline checks/CI

- [x] Restore a clean frontend type-check baseline.
  - Completed: dependency-update compatibility fixes removed the `matchAll` unknown type issue, `InstalledPage.svelte` declaration ordering issue, clickable-card a11y warning, and TypeScript 6 `baseUrl` warning while preserving Wails aliases.
  - Verification: after Wails-generated bindings are present, `npm --prefix frontend run check` reports 0 errors and 0 warnings.
- [x] Make clean-checkout verification deterministic when generated Wails artifacts are absent.
  - Completed: `scripts/verify.sh` regenerates Wails bindings and `frontend/dist` through Wails, then runs frontend type checks and Go tests.
  - Verification: README/CONTRIBUTING document the script as the clean-checkout check path and reiterate that generated files are not hand-edited.
- [x] Add frontend type checking to CI once generated bindings are available.
  - Completed: CI now runs `npm --prefix frontend run check` after the Wails build step, where generated bindings are available.
  - Verification: local `./scripts/verify.sh` exercises the same generated-bindings-before-type-check sequence.

### Operations/docs

- [x] Fix or intentionally document the pprof environment variable name.
  - Completed: `SCRIBE_PPROF=1` is now the documented profiling switch, with `SCRIBEEGO_PPROF=1` retained as a legacy alias.
  - Verification: `pprof_test.go` covers both env names and the disabled default.

## P2 — Backend correctness and data/persistence regression coverage

Purpose: reduce update/matching/cache/settings regressions before broader distribution. Risk level: medium-high because most domain behavior is untested. Scope guardrail: use temp dirs/temp SQLite and mocked/fixture data; do not require live ESOUI or a real AddOns directory.

### Parser/scanner/matcher tests

- [x] Add parser tests for ESO manifest metadata edge cases.
  - Completed: scanner parser tests now cover required/PC/optional dependency fields with version operators, ignored console dependencies, color-code stripping, fallback title, saved variables, API/addon versions, and library boolean forms.
  - Verification: `go test ./internal/scanner` passes.
- [x] Add matcher/version tests for update detection and remote directory selection.
  - Completed: matcher tests now cover exact/equal versions, local older/newer, empty versions, prefixed/suffixed version strings, most-specific remote candidate selection for multi-dir addons, and unmatched locals.
  - Verification: `go test ./internal/esoui` passes.
- [x] Add dependency-resolution tests for missing dependency discovery.
  - Completed: app-level missing dependency tests cover versioned dependency tokens, required-over-optional precedence, installed dependencies being ignored, remote UID mapping by `UIDirs`, and unresolved dependencies marked not installable.
  - Verification: `go test .` passes.

### Persistence/data

- [x] Add cache database round-trip and schema-invalidation tests.
  - Completed: temp SQLite cache tests cover persisted feed URLs, addon/category JSON fields, Set/Get reload, stale detection, explicit `Invalidate`, and schema mismatch deletion.
  - Verification: `go test ./internal/esoui` passes.
- [x] Add settings persistence tests.
  - Completed: temp SQLite settings tests cover defaults, save/load, invalid memory/theme fallback, addon path round trip, rejected invalid paths, inert auto-update, and repeated saves updating existing rows.
  - Verification: `go test ./internal/settings` passes.
- [x] Add install MD5 record tests and update-suppression coverage.
  - Completed: install MD5 tests cover save/read for multiple UIDs, empty/nil no-ops, existing UID updates, and update suppression behavior when stored and remote MD5 values match or differ.
  - Verification: `go test ./internal/esoui .` passes.

### Network/API

- [x] Add MMOUI/ESOUI client fixture tests for API conversion and retry behavior.
  - Completed: `httptest` client fixtures cover active/inactive global config, inactive/missing ESO game, 5xx retry, non-200 failure, malformed JSON, date/count parsing, category parent IDs, and details URL formation.
  - Verification: `go test ./internal/esoui` passes.

## P3 — Frontend UX/accessibility and interaction polish

Purpose: improve user trust, accessibility, and clarity once critical correctness is stable. Risk level: medium. Scope guardrail: prefer focused UI changes; do not replace the route/store architecture.

### Frontend/UX

- [x] Improve feedback when remote refresh fails but stale cache exists.
  - Completed: backend now exposes remote catalog status, including cached-data presence, staleness, and the last background refresh error; `FindMorePage.svelte` distinguishes stale cached data, failed background refreshes, and no saved catalog data.
  - Verification: Linux Wails build, `npm --prefix frontend run check`, `go test ./...`, and `git diff --check` pass.
- [x] Review `OpenPath` for least-surprise behavior.
  - Completed: `App.OpenPath` now validates that the requested directory is the configured AddOns directory or a child directory after symlink resolution before invoking OS shell helpers.
  - Verification: `open_path_test.go` covers addon root/child directories, empty/relative paths, outside and sibling-prefix directories, missing targets, file targets, and symlink escapes; Linux Wails build, `npm --prefix frontend run check`, `go test ./...`, and `git diff --check` pass.
- [x] Improve keyboard/a11y coverage for clickable rows and custom controls.
  - Completed: installed and remote addon rows now support focus, Enter/Space activation, and keyboard context menus; context menus focus the first enabled item and support Escape/arrow/Home/End navigation; category clear controls are keyboard reachable; dialogs and the lightbox expose dialog semantics/focus while remaining suppressions are documented for backdrop/rich-text delegation.
  - Verification: `npm --prefix frontend run check`, `npm --prefix frontend run build`, Linux Wails build, `go test ./...`, and `git diff --check` pass.
- [x] Prevent accidental duplicate install/update queues from rapid UI actions.
  - Completed: download and remote stores now keep per-UID pending guards, normalize/dedupe batch UID lists, skip already queued/downloading/extracting UIDs before calling Wails, and expose per-UID install state so row/detail/update actions no longer globally block unrelated installs.
  - Verification: `npm --prefix frontend run check`, `npm --prefix frontend run build`, Linux Wails build, `go test ./...`, and `git diff --check` pass.

### Frontend tests

- [x] Add a minimal frontend smoke-test strategy for core state flows.
  - Completed: Vitest is configured via `npm --prefix frontend run test`; smoke tests cover install UID dedupe/filtering and the remote catalog status service with mocked Wails wrappers and no live ESOUI dependency.
  - Verification: `npm --prefix frontend run test`, `npm --prefix frontend run check`, `npm --prefix frontend run build`, Linux Wails build, `go test ./...`, and `git diff --check` pass.

## P4 — Documentation, contributor experience, and local tooling

Purpose: make setup, verification, release expectations, and generated-file recovery reliable for maintainers. Risk level: medium-low. Scope guardrail: docs should reflect implemented behavior only.

### Docs

- [x] Document generated-file recovery in README/CONTRIBUTING/frontend README.
  - Completed: README, CONTRIBUTING, and `frontend/README.md` now explain that `frontend/wailsjs/` and `frontend/dist/` are Wails-generated, when to use `wails dev`/`wails build` to recover them, and that generated files are not hand-edited.
  - Verification: `git diff --check` passes.
- [x] Align documented check commands with actual project gates.
  - Completed: README, CONTRIBUTING, and `frontend/README.md` now list the Go, frontend check/test/build, Wails packaging, clean-checkout generated-file caveats, Linux `webkit2_41` build tag, and the mutating ESLint caveat.
  - Verification: `git diff --check` passes.
- [x] Document local database/cache location and reset behavior.
  - Completed: README and CONTRIBUTING now document `Scribe/esoui_cache.db` under the OS user config directory, what the database stores, that it is separate from ESO AddOns, and the safe close-and-rename/delete reset flow.
  - Verification: `git diff --check` passes.

### Tooling/scripts

- [x] Add a non-mutating verification script or Make target for common checks.
  - Completed: `scripts/verify.sh` now runs `git diff --check`, the Wails build/regeneration path, frontend type checks, frontend smoke tests, and Go tests without invoking mutating lint/format commands; docs describe the command as the common clean-checkout verification path.
  - Verification: `./scripts/verify.sh` passes.
- [x] Add a non-fixing lint script if linting is intended in CI or local checks.
  - Completed: `frontend/package.json` now exposes `lint:check` as `eslint .`, while the existing mutating `lint` command remains available for intentional autofixes; README, CONTRIBUTING, and `frontend/README.md` recommend `lint:check` for verification.
  - Verification: `npm --prefix frontend run lint:check` and `./scripts/verify.sh` pass.

## P5 — Release/distribution, compatibility, and operations hardening

Purpose: harden the path from version to user-installable artifacts after the app is stable. Risk level: medium-low but important for production distribution. Scope guardrail: do not publish, tag, dispatch workflows, sign, notarize, or require maintainer credentials during implementation unless explicitly requested.

### CI/release

- [x] Add release workflow validation for version/tag consistency.
  - Completed: `release.yml` now has a pre-build validation job that checks `frontend/package.json` is strict `X.Y.Z` and fails before matrix builds unless `RELEASE_TAG` equals `v<package version>`.
  - Verification: `git diff --check` passes.
- [x] Decide whether automatic tag creation on every push to `main` is intended.
  - Completed: `tag-release.yml` is now manual-only via `workflow_dispatch`; README and CONTRIBUTING document that release tagging is a maintainer-controlled action, not an automatic side effect of routine `main` pushes.
  - Verification: `git diff --check` passes.
- [x] Verify packaged artifact names and installer expectations across platforms.
  - Completed: release docs now classify Windows portable, Linux binary, and macOS universal zip as mandatory and the Windows NSIS installer as optional; `release.yml` validates mandatory staged assets with `test -s` and logs whether the optional installer was produced.
  - Verification: `git diff --check` passes.

### Compatibility/operations

- [x] Add platform-specific path detection tests or fixtures.
  - Completed: `scanner.DetectAddonPath` now delegates to an injected helper for home/GOOS/filesystem/glob behavior; tests cover Windows live/liveeu precedence and OneDrive glob matches, macOS Documents live, Linux Steam compatdata precedence/fallback, and unsupported or missing paths without using real user directories.
  - Verification: `go test ./internal/scanner` and `./scripts/verify.sh` pass.
- [x] Document or test Linux build dependency requirements against current Wails/WebKit tags.
  - Completed: README and CONTRIBUTING document Linux Wails packages for Debian/Ubuntu and Fedora, including the `webkit2_41` local build tag.
  - Verification: CI and release workflows install the same full Ubuntu native toolchain set before Linux Wails builds.

## P6 — Performance, observability, and maintainability improvements

Purpose: keep startup/memory responsive and make performance/debug data actionable after functional gaps are closed. Risk level: low-to-medium. Scope guardrail: measure before optimizing; keep existing sticky TanStack Query behavior unless intentionally changed.

### Performance/observability

- [ ] Add regression guidance or checks for startup and memory budgets.
  - Evidence: `app.go` records startup/memory diagnostics with targets `<1s` frontend-ready and `<=150 MB` system memory, but no automated check or documented manual benchmark process exists.
  - Acceptance criteria: docs or tests define how to capture baseline snapshots, what data to include in PRs, and what threshold changes require maintainer approval.
- [ ] Review remote refresh concurrency and duplicate background refreshes.
  - Evidence: `GetRemoteAddons` can start a background refresh whenever cache is stale and list is non-empty; there is no in-flight guard beyond `refreshWg` wait on shutdown.
  - Acceptance criteria: repeated frontend queries while stale do not spawn redundant remote refreshes; behavior is covered by a focused test or guarded implementation.
- [ ] Add diagnostics/logging for failed cache DB initialization without exposing private paths unnecessarily.
  - Evidence: startup silently falls back in some paths when DB/cache/settings initialization fails, while docs do not describe troubleshooting persistent DB failures.
  - Acceptance criteria: failures are logged with actionable, privacy-conscious messages and the UI can still explain degraded persistence/cache behavior.

### Maintainability

- [ ] Extract testable helpers from `App` methods that currently require Wails app state.
  - Evidence: behavior such as missing dependency aggregation and MD5 false-positive suppression lives on `App`, making it harder to unit test without constructing Wails-adjacent state.
  - Acceptance criteria: small pure/internal helpers are introduced where needed, with tests, without changing the Wails binding surface.

## P7 — Deferred/future scope (not active production-readiness work)

Purpose: record explicitly deferred ideas so they are not confused with current commitments. Risk level: intentionally deferred/out of scope. Scope guardrail: do not implement unless the maintainer explicitly requests a scoped change.

- [ ] Alternate addon sources beyond ESOUI/MMOUI.
  - Evidence: README “When not to use this” excludes addon sources outside ESOUI/MMOUI; `internal/esoui/client.go` only bootstraps MMOUI/ESOUI.
  - Acceptance criteria: only reconsider with a product decision, source-specific safety model, and tests/fixtures.
- [ ] Account/cloud sync.
  - Evidence: README excludes cloud sync/accounts and no account/auth code exists.
  - Acceptance criteria: only reconsider with explicit product requirements and privacy/security design.
- [ ] Telemetry/analytics.
  - Evidence: project scope excludes telemetry and no telemetry code was found.
  - Acceptance criteria: only reconsider with opt-in privacy design and maintainer approval.
- [ ] Plugin APIs or broad architecture rewrites.
  - Evidence: AGENTS and docs scope Scribe as a small Wails/Svelte desktop app without plugin APIs.
  - Acceptance criteria: only reconsider through a separate design plan.
- [ ] Strong distribution signing/notarization.
  - Evidence: README documents unsigned Windows builds and ad-hoc/non-notarized macOS builds; release workflow performs ad-hoc macOS signing only.
  - Acceptance criteria: requires maintainer credentials/certificates and explicit release-scope approval.

## Completed / current-state evidence

- [x] Wails desktop app boundary exists and delegates domain work to internal packages.
  - Evidence: `app.go` binds methods on `App`; scanner, ESOUI/cache/install/download, and settings logic live under `internal/` packages.
- [x] ESOUI/MMOUI is the only implemented remote addon source.
  - Evidence: `internal/esoui/client.go` bootstraps `https://api.mmoui.com/v3/globalconfig.json`; no alternate remote source implementation was found.
- [x] Installed addon scanning prefers canonical folder-named manifests over stubs.
  - Evidence: `internal/scanner/scanner.go` checks `Folder.addon`/`Folder.txt` first; `TestScanAddonDir_PrefersFolderNameManifest` covers this.
- [x] Basic scanner fallback coverage exists.
  - Evidence: `TestScanAddonDir_FallsBackToAlphabetical` covers a non-canonical manifest fallback in a temp addon directory.
- [x] Remote catalog cache is SQLite-backed and schema-versioned.
  - Evidence: `internal/esoui/cache.go` stores addons/categories/meta in `esoui_cache.db` under the historical `Scribe` config dir with `cacheSchemaVersion = "2"`.
- [x] Settings/cache/search presets/install MD5 records share one app DB with GORM migrations.
  - Evidence: `OpenDB` automigrates remote addons, categories, cache meta, settings, search presets, and install records.
- [x] Frontend uses generated Wails bindings through thin service wrappers.
  - Evidence: services under `frontend/src/lib/services` call `callWails`/dynamic Wails imports; `frontend/wailsjs` is absent and treated as generated.
- [x] Route components are dynamically loaded except the initial Installed page.
  - Evidence: `frontend/src/App.svelte` imports `InstalledPage` directly and lazy-loads Find More, Updates, and Settings with dynamic imports.
- [x] Sticky desktop query caching is configured.
  - Evidence: `frontend/src/lib/db/client.ts` sets long `staleTime`/`gcTime` and disables focus/reconnect refetch loops.
- [x] Basic release automation builds Windows, Linux, and macOS assets from version tags.
  - Evidence: `.github/workflows/release.yml` builds a matrix for Windows, Linux, and macOS; `.github/workflows/tag-release.yml` reads `frontend/package.json` version as `vX.Y.Z`.
- [x] Docs disclose important current distribution limitations.
  - Evidence: README notes unsigned Windows builds, Linux UPX use, ad-hoc/non-notarized macOS builds, ESOUI/MMOUI dependency, and intentionally excluded sources/cloud/plugin scope.
