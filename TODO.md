# Scribe TODO Audit Ledger

Last audit refresh: 2026-05-18

## Audit scope inspected

- Code: `app.go`, `main.go`, `pprof.go`, `internal/addon`, `internal/scanner`, `internal/esoui`, `internal/settings`, and the Svelte frontend under `frontend/src` including routes, components, stores, services, query helpers, theme/runtime/diagnostics flows, and utilities.
- Tests: Go coverage now spans scanner parsing/path detection, ESOUI cache/client/install/download behavior, settings persistence, root app safety helpers, and missing-dependency/MD5 helpers; frontend smoke tests run through Vitest for store/service flows alongside `svelte-check`.
- Docs/plans: `AGENTS.md`, `README.md`, `CONTRIBUTING.md`, `frontend/README.md`, and the prior `TODO.md`. No `docs/`, `plan/`, `plans/`, `roadmap/`, `roadmaps/`, or `backlog/` directories/files were present outside this ledger.
- Scripts/config: `go.mod`, `go.sum`, `wails.json`, `frontend/package.json`, `frontend/package-lock.json`, `frontend/vite.config.ts`, `frontend/svelte.config.js`, `frontend/eslint.config.js`, `scripts/build-release.sh`, `scripts/build-release.ps1`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`, and `.github/workflows/tag-release.yml`.
- Generated/runtime surfaces: `frontend/wailsjs/`, `frontend/dist/`, `build/bin/`, build reports, and packaged binaries are treated as generated; Wails build/regeneration is the supported recovery path.
- Verification baseline: `./scripts/verify.sh` runs diff sanity, Wails build/regeneration, frontend type checks, frontend smoke tests, and Go tests without mutating lint/format commands.

## Current direction

- Keep the production app Wails/Svelte/Go while improving speed, stability, install reliability, dependency handling, and day-to-day desktop feel.
- Do not revive the removed Avalonia/native rewrite unless a new accepted plan explicitly asks for it.
- Treat SolidJS, Electron, Tauri, custom WebKit, or other desktop/frontend options as measured spikes only. The goal is a better app, not a framework swap for its own sake.
- Prioritize evidence from benchmarks, diagnostics, UI profiling, and real user workflows before large changes.
- Keep Scribe local-first and ESOUI/MMOUI-only.

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

## P9 — Wails-first performance, stability, and desktop experience backlog (open)

Purpose: make the current app lighter, smoother, less crash-prone, and more predictable before considering any framework or desktop-shell migration. Risk level: medium-high because these touch core install, catalog, startup, and rendering paths. Scope guardrail: keep changes incremental, measured, ESOUI/MMOUI-only, and compatible with the current Wails app.

### Measurement and baselines

- [x] Record a clean performance baseline before more optimization.
  - Evidence: existing diagnostics and benchmark scripts exist, but future work needs current cold/warm startup, memory, catalog, search, scroll, and install-progress numbers to avoid guesswork.
  - Acceptance criteria: run or document `./scripts/verify.sh`, `./scripts/benchmarks.sh`, frontend catalog benchmarks, cold/warm diagnostics exports, and a short manual Find More scroll/search profile; record baseline numbers or a redacted summary in this ledger or dedicated docs.
  - Completed: `docs/performance-baseline.md` records the current scanner, matcher, cached catalog, backend remote search, frontend catalog benchmark, and generated bundle-report baseline; it also documents the cold/warm diagnostics and manual Find More profile capture procedure for real desktop sessions.
  - Verification: `./scripts/benchmarks.sh` captured fixture-backed Go and frontend benchmark values without live ESOUI or real AddOns directories.
- [x] Add frontend interaction timing probes for Find More search/filter/sort and task-center updates.
  - Evidence: Find More does catalog preparation, search scoring, category counting, sorting, and virtual-list updates; task progress writes frequent bridge events into reactive state.
  - Acceptance criteria: local diagnostics can capture search/filter duration, visible list size, result count, progress event rate, and dropped/error states without telemetry or network upload.
  - Completed: frontend diagnostics now record Find More filter/sort timing metadata, visible virtual-list size, result count, and download progress event totals/rates/error counts; Settings diagnostics and the local diagnostics export include the captured values.
  - Verification: `frontend/src/lib/diagnostics/frontend-perf.test.ts` covers timing, gauge, and progress-event snapshots.
- [x] Add a repeatable UI smoke/profile script for desktop workflows.
  - Evidence: current tests cover stores/services, but not real navigation, modal opening, search typing, scroll behavior, or task-center interaction inside the rendered app.
  - Acceptance criteria: scripted local workflow covers Installed, Find More, Updates, Settings, addon details, dependency banners, task center, and failure/retry states using mocks or fixture data where possible.
  - Completed: `scripts/profile-ui-workflows.sh` now runs fixture-backed frontend workflow tests, catalog benchmarks, and a production frontend build, then writes an ignored Markdown report mapping coverage across Installed, Find More, Updates, Settings, addon detail data, dependency banners, task center, and failure/retry states.
  - Verification: README and CONTRIBUTING document the script and its real-desktop smoke-test limitation.

### Startup and cache responsiveness

- [x] Move initial AddOns scan off the startup critical path.
  - Evidence: `startup()` currently detects and scans AddOns before database setup and async ESOUI initialization, which can delay first useful UI on large folders.
  - Acceptance criteria: app can render quickly with cached/last-known state, then refresh installed addons asynchronously; diagnostics distinguish frontend-ready, cached-state-ready, scan-start, scan-ready, and remote-ready timing.
  - Completed: startup now configures the scanner path without parsing the AddOns tree, `GetInstalledAddons` returns cached state while starting a background scan, the frontend refreshes installed/matched query state on the `installed:scan-complete` event, and diagnostics now expose cached-state-ready, scan-start, scan-ready, in-flight, and scan-error fields.
  - Verification: `startup_scan_test.go` proves `GetInstalledAddons` returns cached state immediately while the background scan later populates the scanner cache.
- [x] Add incremental scanner caching for unchanged addon folders.
  - Evidence: full rescans reparse manifests even when folder mtimes/sizes have not changed.
  - Acceptance criteria: scanner stores safe per-folder metadata in the app DB or a cache table, reparses changed folders only, invalidates correctly on folder deletion/rename, and preserves canonical-manifest preference tests.
  - Completed: scanner now fingerprints addon manifest files per folder, reuses cached parsed addons when fingerprints match, persists the cache in the app SQLite DB through `scanner_cache`, and replaces cache rows for the active AddOns path on each successful scan.
  - Verification: scanner tests cover reuse for unchanged folders, cache round-trip tests cover the SQLite-backed scanner cache, and existing canonical/fallback manifest tests remain in place.
- [x] Make remote catalog refresh more visibly background-first.
  - Evidence: stale cache handling exists, but the UI should never feel empty or blocked when a usable cached ESOUI catalog exists.
  - Acceptance criteria: cached remote data appears immediately when available, stale/background refresh status remains visible, refresh failure keeps prior results, and manual refresh does not duplicate in-flight work.
  - Completed: remote catalog status now reports refresh-in-flight and refresh-started timestamps, Find More shows an explicit cached-data/background-refresh banner, manual refresh uses the same in-flight guard, and frontend refresh failures no longer replace existing cached results with an empty list.
  - Verification: backend tests cover refresh in-flight status/guarding, frontend catalog-status tests cover the in-flight cached-data state, and service tests cover normalized status fields.

### Catalog, search, and rendering jank

- [x] Optimize Find More catalog indexing and search ranking.
  - Evidence: search score and lowercase/string/date/version work can be repeated during filtering and sorting.
  - Acceptance criteria: create a tested pure catalog index that precomputes lowercase title/author/folder fields, category name/order, latest compatibility, date number, library-like flag, and stable sort keys; sorting reuses per-query search scores instead of recomputing them in comparators.
  - Completed: Find More now builds a pure indexed remote catalog with precomputed lowercase title/author/folder fields, category names/order, latest compatibility, date timestamps, library-like flags, icon metadata, compatibility versions, and stable sort inputs; filtering computes each query score once per candidate and sorting reuses that score.
  - Verification: `remote-catalog-index.test.ts` covers indexing fields, search ranking, installed/content/version/category filters, category counts, sort keys, and game-version option sorting; frontend catalog benchmarks now include indexed filter/sort timing.
- [ ] Add a worker-backed catalog filtering spike if main-thread filtering still exceeds frame budget.
  - Evidence: a large ESOUI catalog can make search/filter/sort CPU-bound even with virtualization.
  - Acceptance criteria: timeboxed Web Worker or worker-like spike processes fixture catalog search/filter/sort off the UI thread and is kept only if it measurably improves responsiveness without complicating state flow.
- [ ] Tighten virtual-list and image behavior.
  - Evidence: virtualized lists exist, but image load, item measurement, overscan, and details preloading can still cause scroll hitching.
  - Acceptance criteria: list rows keep stable dimensions, lazy images have fixed boxes and fallbacks, overscan is bounded, image/detail prefetch is limited, and scroll remains smooth on large installed and remote lists.
- [ ] Bound addon detail and screenshot cache memory.
  - Evidence: details and screenshots are useful but can accumulate memory across browsing sessions.
  - Acceptance criteria: detail queries/images use an explicit bounded cache or eviction policy, memory cleanup has deterministic behavior, and diagnostics expose detail cache size/count.

### Install, update, and task-center smoothness

- [ ] Coalesce download progress events before reactive store writes.
  - Evidence: backend emits progress every 100 ms per active task, and the frontend currently writes each event into a reactive map.
  - Acceptance criteria: state transitions remain immediate, byte/progress updates are batched with `requestAnimationFrame` or a measured throttle, task center remains responsive during concurrent downloads, and cancel/retry behavior is unchanged.
- [ ] Make backend progress interval adaptive.
  - Evidence: byte-level progress does not need the same frequency as state changes, especially with multiple downloads.
  - Acceptance criteria: planning/downloading/extracting/complete/failed/cancelled transitions emit immediately; byte progress is throttled to a measured interval such as 200-250 ms unless a benchmark proves otherwise.
- [ ] Improve install/update preflight presentation.
  - Evidence: archive preflight exists, but users need clearer confidence before mutating AddOns.
  - Acceptance criteria: install/update confirmation shows folders to add/replace, dependency impact, expected download size when known, rollback behavior, and any warning that blocks install before mutation.
- [ ] Clean stale temporary install/update artifacts on startup.
  - Evidence: rollback-safe staging and backups exist; crashes or forced exits can still leave temporary folders.
  - Acceptance criteria: startup detects Scribe-owned stale staging/backup folders under AddOns, presents or safely cleans only app-owned temp artifacts, never deletes user addon folders, and logs privacy-safe diagnostics.

### Dependency and update experience

- [x] Ensure missing dependency installs use the latest canonical ESOUI addon entry.
  - Evidence: dependency folder resolution could map a folder to whichever ESOUI catalog entry was seen last when duplicate `UIDirs` existed, including older bundled entries.
  - Acceptance criteria: duplicate remote folder candidates prefer the canonical/single-folder addon entry, then the newest catalog date/version before queueing; dependency version constraints remain visible but do not pin the download to an older release.
  - Completed: dependency resolution and addon matching now share deterministic best-remote selection, and the install path fetches addon details for the selected UID so the queued download comes from that addon page's latest details.
  - Verification: `missing_dependencies_test.go` covers old bundled vs latest canonical dependency resolution; `internal/esoui/matcher_test.go` covers best remote selection.
- [ ] Improve missing dependency UX for required vs optional libraries.
  - Evidence: dependency planning exists, but install affordances should be clearer and less noisy.
  - Acceptance criteria: required dependencies are prioritized, optional dependencies are grouped separately, version constraints are visible, unresolved dependencies explain why no install action exists, and batch install dedupes already active tasks.
- [ ] Add update explanation details everywhere update actions appear.
  - Evidence: update-state reasons now exist, but rows, detail views, and task flows should consistently explain remote-newer, local-newer, MD5-only changed, unknown-version, and unmatched states.
  - Acceptance criteria: Installed, Updates, addon detail, and task planning surfaces share one tested formatter for update reason text and safe action eligibility.
- [ ] Add recovery guidance for failed installs and partial failures.
  - Evidence: retry exists, but users need useful next steps when download, MD5, archive preflight, extraction, or commit fails.
  - Acceptance criteria: failed task details classify the stage, show a short safe action, include copyable diagnostics, and never suggest manually deleting broad AddOns directories.

### Desktop shell and frontend framework evaluation

- [ ] Document a Wails vs Tauri vs Electron vs SolidJS evaluation matrix.
  - Evidence: the user is open to Electron or other desktop shells if they improve stability and experience, but Electron is not automatically lighter and framework swaps do not fix backend/cache/install issues.
  - Acceptance criteria: matrix compares startup, memory, package size, Linux/Fedora dependencies, Windows behavior, webview/runtime ownership, packaging complexity, native API access, Wails bridge replacement cost, and regression risk.
- [ ] Run a SolidJS frontend spike only after Svelte hot paths have baselines.
  - Evidence: Solid has fine-grained reactivity, but current Svelte 5 code already uses runes, lazy chunks, TanStack Query, and virtualization.
  - Acceptance criteria: isolated spike ports the shell plus Find More catalog list against the existing Go/Wails service shape or mocked services; compare bundle size, search latency, scroll smoothness, memory, and migration cost before deciding.
- [ ] Run an alternate desktop-shell spike only if Wails itself is proven to be the bottleneck.
  - Evidence: WebKitGTK packaging friction and runtime behavior matter on Linux, but replacing Wails means replacing bindings, build/release flow, and native integrations.
  - Acceptance criteria: spike proves a concrete Wails limitation with measurements, then compares Tauri/Electron/custom shell using the same fixture workflows; no app rewrite begins without an accepted migration plan.

### Maintainability and anti-overengineering

- [ ] Move hot route logic into tested pure helpers without adding broad architecture layers.
  - Evidence: large route components are harder to profile and test, but a heavy abstraction rewrite would add risk.
  - Acceptance criteria: extract only catalog indexing, update reason formatting, dependency display planning, and task summary shaping where tests/benchmarks justify it.
- [ ] Move user-facing settings from SQLite into an atomic `settings.toml` file while keeping SQLite for cache/state.
  - Evidence: SQLite is appropriate for ESOUI catalog cache, category data, install MD5 records, and keyed app state, but small human-facing settings such as AddOns path, theme, and diagnostics thresholds are easier to inspect, recover, and edit safely as TOML.
  - Acceptance criteria: app stores settings at the existing user config dir as `settings.toml`, keeps `esoui_cache.db` for catalog/cache/install records, validates TOML settings before use, writes via temp-file-plus-rename, migrates existing SQLite settings to TOML once without losing AddOns path/theme/memory values, and has temp-dir tests for fresh settings, migration, invalid TOML fallback, and atomic write failure behavior.
- [ ] Audit dependencies for real use and runtime impact.
  - Evidence: current `node_modules` has extraneous packages locally, and package churn can obscure real performance work.
  - Acceptance criteria: verify usage before removal, clean install with npm, compare build report/lockfile effects, and keep dependencies that solve real problems such as virtualization/query caching.
- [ ] Add coding-pattern notes to docs after each performance fix.
  - Evidence: performance regressions often come from repeated hot-path mistakes.
  - Acceptance criteria: update `AGENTS.md`, README, or CONTRIBUTING only with durable lessons such as progress-event batching, catalog index reuse, startup scan boundaries, and generated-file recovery rules.

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
