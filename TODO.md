# Scribe TODO Audit Ledger

Last audit refresh: 2026-05-18

## Audit scope inspected

- Code: `app.go`, `main.go`, `pprof.go`, `internal/addon`, `internal/scanner`, `internal/esoui`, `internal/settings`, and the Svelte frontend under `frontend/src` including routes, components, stores, services, query helpers, theme/runtime/diagnostics flows, and utilities.
- Tests: Go coverage now spans scanner parsing/path detection, ESOUI cache/client/install/download behavior, settings persistence, root app safety helpers, and missing-dependency/MD5 helpers; frontend smoke tests run through Vitest for store/service flows alongside `svelte-check`.
- Docs/plans: `AGENTS.md`, `README.md`, `CONTRIBUTING.md`, `frontend/README.md`, and the prior `TODO.md`. No `docs/`, `plan/`, `plans/`, `roadmap/`, `roadmaps/`, or `backlog/` directories/files were present outside this ledger.
- Scripts/config: `go.mod`, `go.sum`, `wails.json`, `frontend/package.json`, `frontend/package-lock.json`, `frontend/vite.config.ts`, `frontend/svelte.config.js`, `frontend/eslint.config.js`, `scripts/build-release.sh`, `scripts/build-release.ps1`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`, and `.github/workflows/tag-release.yml`.
- Generated/runtime surfaces: `frontend/wailsjs/`, `frontend/dist/`, `build/bin/`, build reports, and packaged binaries are treated as generated; Wails build/regeneration is the supported recovery path.
- Verification baseline: `./scripts/verify.sh` runs diff sanity, Wails build/regeneration, frontend type checks, frontend smoke tests, and Go tests without mutating lint/format commands.

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

- [x] Add regression guidance or checks for startup and memory budgets.
  - Completed: README and CONTRIBUTING now document the Settings diagnostics panel as the baseline capture path, the frontend-ready `<1000 ms` and Go `Sys <=150 MB` targets, required PR snapshot fields, warm/cold catalog context, and maintainer approval for threshold changes.
  - Verification: `git diff --check` passes.
- [x] Review remote refresh concurrency and duplicate background refreshes.
  - Completed: stale cached `GetRemoteAddons` calls now use a remote-refresh in-flight guard before starting the background refresh goroutine, and the guard is released when refresh work exits.
  - Verification: `remote_refresh_test.go` covers the guard preventing duplicate starts while in flight and allowing another refresh after completion; `go test .` and `./scripts/verify.sh` pass.
- [x] Add diagnostics/logging for failed cache DB initialization without exposing private paths unnecessarily.
  - Completed: app startup now records degraded persistence when the shared settings/cache DB cannot open, logs a privacy-safe permissions/disk-space message instead of local paths, and surfaces persistence status/error in Settings diagnostics.
  - Verification: `persistence_test.go` covers path redaction and diagnostics status; `go test .`, `npm --prefix frontend run check`, and `./scripts/verify.sh` pass.

### Maintainability

- [x] Extract testable helpers from `App` methods that currently require Wails app state.
  - Completed: missing dependency aggregation now lives in the pure `findMissingDependencies` helper, while `GetMissingDependencies` remains the Wails-facing scanner wrapper; existing MD5 suppression already uses the pure `suppressMD5Matches` helper.
  - Verification: `missing_dependencies_test.go` covers the pure helper and the App wrapper without changing the Wails binding surface; `go test .` and `./scripts/verify.sh` pass.

## P7 — Deferred/future scope (not active production-readiness work)

Purpose: record explicitly deferred ideas so they are not confused with current commitments. Risk level: intentionally deferred/out of scope. Scope guardrail: do not implement unless the maintainer explicitly requests a scoped change.

- [x] Alternate addon sources beyond ESOUI/MMOUI.
  - Closed as not implementable under current product constraints: research did not identify a second ESO addon source that can provide equivalent catalog/search/install/update/download behavior without credentials, non-equivalent source semantics, duplicate-addon matching risk, or a new trust/safety model.
  - Evidence: ESOUI/MMOUI remains Scribe's canonical source; other ESO addon managers found still rely on ESOUI/direct ESOUI downloads, while CurseForge/Nexus require API/key/policy decisions and are not equivalent ESOUI replacements.
  - Decision: keep Scribe ESOUI/MMOUI-only unless a future product decision names a specific non-equivalent source and accepts its API, credential, duplication, and safety trade-offs.
- [x] Account/cloud sync.
  - Closed as not implementable under current product constraints: Scribe has no account system, authentication boundary, hosted service, conflict model, or privacy/security design for syncing addon state.
  - Evidence: README excludes cloud sync/accounts, no account/auth code exists, and local settings/cache are intentionally stored in the user config DB.
  - Decision: keep Scribe local-first unless a future product plan defines account identity, sync storage, conflict handling, deletion/export semantics, and privacy/security requirements.
- [x] Telemetry/analytics.
  - Closed as not implementable under current product constraints: telemetry would require explicit opt-in UX, a privacy design, event schema, retention policy, endpoint ownership, and maintainer approval.
  - Evidence: no telemetry code exists; README now states Scribe has no telemetry/analytics and limits network behavior to MMOUI/ESOUI catalog/downloads plus user-triggered external links.
  - Decision: keep Scribe telemetry-free unless a future approved privacy plan defines exactly what is collected, where it goes, how users opt in/out, and how data is retained/deleted.
- [x] Plugin APIs or broad architecture rewrites.
  - Closed as not implementable under current product constraints: plugin APIs and broad rewrites conflict with the small focused Wails app boundary unless a separate design plan justifies the surface, lifecycle, compatibility, and security model.
  - Evidence: AGENTS and docs scope Scribe as a small Wails/Svelte desktop app without plugin APIs; CONTRIBUTING now rejects plugin API or architecture rewrite work without an accepted design plan.
  - Decision: keep the current Wails/app.go/internal package boundary and require a separate accepted design before reconsidering plugin APIs or large architecture changes.
- [x] Strong distribution signing/notarization.
  - Closed as not implementable in this repo session: strong Windows signing and macOS notarization require maintainer-owned certificates, Apple credentials, secret handling, and release approval that are intentionally unavailable to normal implementation work.
  - Evidence: README documents unsigned Windows builds and ad-hoc/non-notarized macOS builds; release workflow performs ad-hoc macOS signing only; CONTRIBUTING now blocks signing/notarization automation without defined credentials and release approval.
  - Decision: keep current unsigned/ad-hoc distribution disclosure until maintainers provide credentials, secret-management rules, and an explicit release-scope implementation request.

## P8 — Next app-quality backlog (open)

Purpose: improve the app without replacing Wails/Svelte/Go: fewer crashes, smoother UX, better install/update/dependency outcomes, stronger discovery, and measurable performance. Risk level: medium because these affect core workflows. Scope guardrail: keep improvements incremental, local-first, ESOUI/MMOUI-only, and compatible with current generated-file and release rules.

### Crash resistance and recovery

- [x] Add a frontend route/service error boundary with user-facing recovery actions.
  - Completed: `App.svelte` now catches mounted route component failures with Svelte boundaries, records lazy route import failures, keeps navigation mounted, and shows a recoverable error state with retry plus copyable details.
  - Verification: `frontend/src/lib/routes/recovery.test.ts` covers failed service error formatting and failed dynamic route retry; `npm --prefix frontend run test`, `npm --prefix frontend run check`, `npm --prefix frontend run build`, and `./scripts/verify.sh` pass.
- [x] Add a local-only redacted diagnostics export.
  - Completed: Settings now exposes a local copy-to-clipboard diagnostics export with app/build/platform data, redacted AddOns paths, startup timings, memory, persistence status, catalog/cache state, frontend cache counters, and recent failed install/update tasks.
  - Verification: `frontend/src/lib/diagnostics/export.test.ts` covers path/error redaction and payload content; `npm --prefix frontend run test` and `npm --prefix frontend run check` pass.
- [x] Audit background goroutine and async task lifecycles for shutdown safety.
  - Completed: remote catalog refresh now skips cache/UI state writes after shutdown cancellation, app shutdown remains idempotent while waiting on background refresh work, and frontend download-store listener shutdown clears delayed dependency-check and invalidation timers.
  - Verification: `shutdown_lifecycle_test.go` covers post-shutdown catalog-write suppression plus repeated shutdown waiting on background work; `go test . ./internal/esoui`, `npm --prefix frontend run check`, and frontend smoke tests pass.

### Install, update, and dependency reliability

- [x] Add install/update archive preflight planning before mutating AddOns.
  - Completed: downloaded archives are preflighted before extraction, validating safe addon folder names, canonical manifests, destination boundaries, and ESOUI `UIDirs`; task progress now emits a `planning` state with add/replace folder actions shown in the download queue.
  - Verification: `internal/esoui/installer_test.go` covers add/replace plans plus root-file, missing-manifest, unexpected-folder, and traversal rejection; `go test . ./internal/esoui`, `npm --prefix frontend run check`, and frontend smoke tests pass.
- [x] Make install/update extraction atomic or rollback-safe.
  - Completed: installs and updates now extract into a temporary staging directory under AddOns, then commit planned folders with backups for replacements and rollback on commit failure; cancellation and invalid archives return before touching existing addon folders and temp staging/backup dirs are cleaned.
  - Verification: `internal/esoui/installer_test.go` covers successful replacement/add, cancellation, invalid archive, and simulated commit failure rollback; `internal/esoui/download_manager_test.go` confirms cancelled extraction leaves no partial addon folder.
- [x] Improve update detection states for ESOUI version and MD5 edge cases.
  - Completed: matched addons now carry explicit `updateState`/`updateReason` values for up-to-date, remote-newer, local-newer, MD5-only changed, unknown-version, and unmatched cases; stored install MD5s can offer same-version download changes while suppressing matching-MD5 false positives, and Updates rows display the state/reason.
  - Verification: `internal/esoui/matcher_test.go` covers version states and unmatched locals; `md5_suppression_test.go` covers MD5 false-positive suppression plus MD5-only changed updates; frontend type checks and smoke tests pass.
- [x] Add a dependency install plan with required/optional/version clarity.
  - Completed: missing-dependency results now include deterministic install plan state, plan reason, version constraints, required-by grouping, required-over-optional precedence, and installable/unresolved classification; banners show the plan before queueing and callers dedupe UIDs before installing only installable dependencies.
  - Verification: `missing_dependencies_test.go` covers shared dependency dedupe, version constraints, installable/unresolved states, installed dependencies being skipped, and required-over-optional behavior; frontend type checks and smoke tests pass.
- [x] Add batch task retry and partial-failure handling.
  - Completed: failed install/update/dependency tasks remain in the download history with their error messages, the queue exposes `Retry failed`, and retry logic requeues only failed non-active UIDs while skipping completed, cancelled, duplicate, or currently active tasks.
  - Verification: `frontend/src/lib/stores/install-queue.test.ts` covers retry filtering for failed-only, dedupe, and active-skip behavior; `npm --prefix frontend run test` and `npm --prefix frontend run check` pass.

### UX smoothness and interaction polish

- [x] Add a persistent task center for active and recent install/update/dependency work.
  - Completed: the route-independent floating queue is now labeled as a task center, summarizes active/recent/retryable work, lists queued/planning/downloading/extracting/complete/failed/cancelled tasks, supports cancel, dismiss, clear, cancel-all, and retry-failed actions, and remains mounted across navigation.
  - Verification: frontend type checks and smoke tests pass; task-center state is held in the shared download store and does not trigger catalog refetches except the existing installed-state refresh after successful installs.
- [x] Reduce list and search jank on large installed and remote catalogs.
  - Completed: Installed and Find More already use TanStack virtualized lists with lazy-loaded images and stable selection state; remote catalog preparation now avoids repeated per-addon compatibility sorts by using a single-pass latest-version helper before filtering/sorting.
  - Verification: `frontend/src/lib/perf/remote-list.test.ts` covers latest compatibility selection without mutating source data; frontend smoke tests and type checks pass.
- [x] Stabilize loading, empty, stale-cache, and error states across core pages.
  - Completed: Installed, Find More, Updates, and Settings now have stable skeleton/loading, empty, error, retry, and diagnostics-not-loaded states; Find More keeps cached ESOUI data visible during stale or failed background refreshes instead of clearing useful results.
  - Verification: code audit confirmed route-level skeletons, dashed empty states, retry actions, stale-cache banners, and stable virtualized list containers across the core pages; `git diff --check` passes.
- [x] Continue keyboard, focus, and context-menu polish for full workflows.
  - Completed: existing route rows, context menus, selects, category controls, dialogs, and lightbox retain keyboard/Escape handling; new task-center and dependency-plan controls use real buttons with explicit types, and the shared dialog close control now has a clear accessible label.
  - Verification: code audit covered the new P8 controls plus existing row/context-menu/dialog flows; frontend type checks pass.

### Catalog, search, and discovery

- [x] Improve offline-first catalog search ranking and filters.
  - Completed: cached catalog search now ranks exact title/folder matches above prefix, loose title/folder, and author matches; Find More adds a libraries/dependencies content filter alongside existing category/version filters and keeps stale cached results visible when refresh fails.
  - Verification: `frontend/src/lib/perf/remote-list.test.ts` covers search ranking and library-like detection; `npm --prefix frontend run test` and `npm --prefix frontend run check` pass.
- [x] Improve addon detail pages for update/install decisions.
  - Evidence: users need enough context to decide whether to install, update, or add optional dependencies without opening the browser for every addon.
  - Acceptance criteria: detail views clearly show installed/remote versions, update reason, dependency status, optional dependency affordances, ESOUI link, cached freshness, and safe install/update actions.
- [x] Add local-only addon health insights.
  - Evidence: Scribe already scans manifests and dependencies; it can surface actionable local issues without cloud features or extra sources.
  - Acceptance criteria: health view flags missing required libraries, outdated-by-metadata addons, orphaned unknown folders, disabled/stub manifests where detectable, and provides safe actions without bulk deleting user folders.
  - Completed: Installed now shows a local health panel derived from installed manifests, matched ESOUI metadata, and missing dependency results. It flags missing required libraries, metadata-reported updates, unknown local folders, and disabled/stub-like manifests where detectable, with safe actions to install required dependencies or queue updates only.
  - Verification: `frontend/src/lib/addons/health.test.ts` covers each issue class and non-destructive action eligibility.

### Performance and maintainability

- [x] Establish repeatable startup, scan, catalog, and memory benchmarks.
  - Evidence: diagnostics budgets exist, but repeatable fixtures make regressions easier to catch before release.
  - Acceptance criteria: benchmark or scripted diagnostics fixtures cover cold startup, warm startup, large AddOns scan, cached catalog load, remote search filtering, and memory snapshots; thresholds are documented before enforcement.
  - Completed: `scripts/benchmarks.sh` now runs fixture-backed Go benchmarks for large AddOns scans, cached catalog load, and backend remote search plus a Vitest benchmark for frontend catalog filtering/ranking. README and CONTRIBUTING document cold/warm startup diagnostics and memory snapshot capture, with fixture thresholds recorded before enforcement.
  - Verification: benchmark files are deterministic temp-dir/in-memory fixtures and avoid live ESOUI or real AddOns paths.
- [x] Profile backend scan/cache hot paths before optimizing.
  - Evidence: scanner, cache load, matching, dependency resolution, and update suppression are the likely hot paths; broad rewrites are out of scope.
  - Acceptance criteria: pprof or benchmark evidence identifies top costs, optimizations are targeted, and tests prove behavior is unchanged.
  - Completed: `scripts/profile-backend.sh` captures CPU and memory pprof output for scanner scans, cached catalog load, matching/search, and dependency resolution into ignored `build/reports/profiles/` files, and prints top costs for review before any optimization work.
  - Verification: the script runs against deterministic benchmark fixtures and avoids live ESOUI, real AddOns folders, or committed profile artifacts.
- [x] Expand frontend smoke tests around real user flows.
  - Evidence: current Vitest coverage is intentionally small; next UX work needs guardrails for store/service interactions.
  - Acceptance criteria: tests cover install/update queue guards, dependency plan confirmation, task retry, stale-cache messaging, error recovery, and route-state preservation with mocked Wails services.
  - Completed: frontend smoke coverage now includes install/update queue dedupe and retry guards, missing dependency plan normalization, matched update-state normalization, stale-cache/no-cache catalog state classification, recoverable route errors, and independent route state preservation using mocked Wails calls or pure helpers.
  - Verification: Vitest covers the added service/catalog/route cases without live ESOUI or generated Wails bindings.

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
  - Evidence: `.github/workflows/release.yml` validates `RELEASE_TAG` against `frontend/package.json` and builds a matrix for Windows, Linux, and macOS; manual `.github/workflows/tag-release.yml` reads `frontend/package.json` version as `vX.Y.Z`.
- [x] Docs disclose important current distribution limitations.
  - Evidence: README notes unsigned Windows builds, Linux UPX use, ad-hoc/non-notarized macOS builds, ESOUI/MMOUI dependency, and intentionally excluded sources/cloud/plugin scope.
