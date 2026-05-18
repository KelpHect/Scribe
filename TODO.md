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

- [ ] Fix or intentionally document the pprof environment variable name.
  - Evidence: `pprof.go` checks `SCRIBEEGO_PPROF`, which appears inconsistent with the app name and is undocumented in README/CONTRIBUTING/frontend docs.
  - Acceptance criteria: the intended env var is documented, or a correctly named variable is added with compatibility for the old spelling.

## P2 — Backend correctness and data/persistence regression coverage

Purpose: reduce update/matching/cache/settings regressions before broader distribution. Risk level: medium-high because most domain behavior is untested. Scope guardrail: use temp dirs/temp SQLite and mocked/fixture data; do not require live ESOUI or a real AddOns directory.

### Parser/scanner/matcher tests

- [ ] Add parser tests for ESO manifest metadata edge cases.
  - Evidence: `internal/scanner/parser.go` parses `DependsOn`, `PCDependsOn`, `OptionalDependsOn`, color codes, `SavedVariables`, `AddOnVersion`, `IsLibrary`, and missing titles, but only scanner manifest selection is tested.
  - Acceptance criteria: table tests cover dependency fields including version operators, `PCDependsOn`, ignored `ConsoleDependsOn`, color-code stripping, fallback title, saved variables, API/addon versions, and library boolean forms.
- [ ] Add matcher/version tests for update detection and remote directory selection.
  - Evidence: `internal/esoui/matcher.go` picks the candidate with fewer `UIDirs` and compares numeric version parts; no tests prove behavior for siblings, multi-dir addons, same versions, empty versions, nonnumeric suffixes, or local-newer versions.
  - Acceptance criteria: tests cover exact matches, local older/newer/equal, version strings with prefixes/suffixes, multi-candidate selection, and no-update false positives.
- [ ] Add dependency-resolution tests for missing dependency discovery.
  - Evidence: `App.GetMissingDependencies` strips simple version operators, lowercases folder names, merges required/optional state, and maps remote dirs, but no tests cover required-vs-optional conflicts, installed deps, versioned dep tokens, or unresolvable deps.
  - Acceptance criteria: tests or extracted helper tests cover required taking precedence over optional, installed deps ignored, remote UID mapping by `UIDirs`, and unresolved deps shown as not installable.

### Persistence/data

- [ ] Add cache database round-trip and schema-invalidation tests.
  - Evidence: `internal/esoui/cache.go` schema-versioned cache persists remote addons/categories/feed URLs/fetched time and invalidates on missing/mismatched schema, but no tests cover these behaviors.
  - Acceptance criteria: temp SQLite tests cover `Set`/`Get` round trip, stale detection, `Invalidate`, schema mismatch deletion, categories/addons JSON fields, and feed URL persistence.
- [ ] Add settings persistence tests.
  - Evidence: `internal/settings/settings.go` has defaults, bool/int parsing, theme fallback, and upsert logic with no direct tests.
  - Acceptance criteria: temp SQLite tests cover defaults, save/load, invalid memory/theme fallback, addon path round trip, and repeated saves updating existing rows.
- [ ] Add install MD5 record tests and update-suppression coverage.
  - Evidence: `SaveInstallMD5`, `GetInstallMD5s`, and `App.suppressMD5FalsePositives` suppress update false positives based on ESOUI MD5, but only persistence helpers exist and are untested.
  - Acceptance criteria: tests cover save/read for multiple UIDs, empty/nil no-ops, and update suppression when stored and remote MD5 match while preserving updates when they differ.

### Network/API

- [ ] Add MMOUI/ESOUI client fixture tests for API conversion and retry behavior.
  - Evidence: `internal/esoui/client.go` converts MMOUI dates/counts/categories/details, retries 5xx responses, and bootstraps feed URLs from `globalconfig.json`, but tests currently do not mock HTTP.
  - Acceptance criteria: `httptest`-based tests cover active/inactive global config, missing ESO game, 5xx retry, non-200 failure, malformed JSON, date/count parsing, category parent IDs, and details URL formation.

## P3 — Frontend UX/accessibility and interaction polish

Purpose: improve user trust, accessibility, and clarity once critical correctness is stable. Risk level: medium. Scope guardrail: prefer focused UI changes; do not replace the route/store architecture.

### Frontend/UX

- [ ] Improve feedback when remote refresh fails but stale cache exists.
  - Evidence: backend serves cached remote lists and refreshes stale data in the background, while `FindMorePage.svelte` shows a generic “Failed to load addons from ESOUI”/“No addons loaded” state without distinguishing cached data from no data.
  - Acceptance criteria: users can tell when they are viewing cached data, when a background refresh failed, and when no data is available, without adding telemetry or alternate sources.
- [ ] Review `OpenPath` for least-surprise behavior.
  - Evidence: `App.OpenPath` opens any frontend-provided path with OS shell helpers; current UI passes addon folder paths, but the bound backend method is broad.
  - Acceptance criteria: either constrain backend calls to configured AddOns descendants for folder-opening operations or document why broader shell opening is intentionally supported.
- [ ] Improve keyboard/a11y coverage for clickable rows and custom controls.
  - Evidence: `svelte-check` warns on `AddonCard.svelte` visible clickable divs; multiple components intentionally suppress Svelte a11y warnings for custom row/list/menu behavior.
  - Acceptance criteria: primary addon rows, remote rows, dialogs, selects, context menus, and lightbox controls are reachable/usable by keyboard and remaining suppressions are justified narrowly.
- [ ] Prevent accidental duplicate install/update queues from rapid UI actions.
  - Evidence: backend deduplicates queued/active UIDs, but frontend sets optimistic queued state before `InstallAddon`/`BatchInstall` and global `remote.installing` can block unrelated installs while still allowing mixed component entry points.
  - Acceptance criteria: rapid clicks and overlapping row/detail/batch installs produce one queued task per UID and UI state remains accurate after backend dedupe.

### Frontend tests

- [ ] Add a minimal frontend smoke-test strategy for core state flows.
  - Evidence: no frontend unit/component/e2e test runner is configured; routes implement install/update/uninstall/settings/search behavior with only static `svelte-check`.
  - Acceptance criteria: a maintainable test setup covers stores/services or critical components with mocked Wails bindings and no live ESOUI dependency.

## P4 — Documentation, contributor experience, and local tooling

Purpose: make setup, verification, release expectations, and generated-file recovery reliable for maintainers. Risk level: medium-low. Scope guardrail: docs should reflect implemented behavior only.

### Docs

- [ ] Document generated-file recovery in README/CONTRIBUTING/frontend README.
  - Evidence: AGENTS records that missing `frontend/dist` breaks `go test ./...` and missing `frontend/wailsjs` breaks frontend checks; README/CONTRIBUTING currently only mention `wails build`/`wails dev` at a high level.
  - Acceptance criteria: contributor docs explain when to run `wails build`/`wails dev`, that `frontend/wailsjs` and `frontend/dist` are generated, and that generated files should not be hand-edited.
- [ ] Align documented check commands with actual project gates.
  - Evidence: `CONTRIBUTING.md` lists only `wails build` and `go test ./...`; AGENTS additionally requires frontend checks for frontend changes and warns `npm run lint` autofixes.
  - Acceptance criteria: docs list Go, frontend, and packaging check commands with clean-checkout caveats and Linux WebKit dependency notes.
- [ ] Document local database/cache location and reset behavior.
  - Evidence: `internal/esoui/cache.go` and settings persistence use the historical user config dir `Scribe/esoui_cache.db`, but user-facing docs do not describe where settings/cache live or how to safely reset cache without touching AddOns.
  - Acceptance criteria: docs describe config DB location by OS at a high level, cache-vs-AddOns distinction, and a safe reset/troubleshooting flow.

### Tooling/scripts

- [ ] Add a non-mutating verification script or Make target for common checks.
  - Evidence: setup/check commands are scattered across docs and AGENTS; `frontend/package.json` has `lint` that runs `eslint . --fix`, which is risky for verification-only use.
  - Acceptance criteria: a documented command runs formatting/diff sanity, frontend check, Wails build as needed, and Go tests without applying automatic source rewrites.
- [ ] Add a non-fixing lint script if linting is intended in CI or local checks.
  - Evidence: current `npm --prefix frontend run lint` invokes `eslint . --fix`, so using it as a check mutates the working tree.
  - Acceptance criteria: `frontend/package.json` exposes a `lint:check` or equivalent that reports lint issues without changing files; docs avoid recommending mutating lint for verification.

## P5 — Release/distribution, compatibility, and operations hardening

Purpose: harden the path from version to user-installable artifacts after the app is stable. Risk level: medium-low but important for production distribution. Scope guardrail: do not publish, tag, dispatch workflows, sign, notarize, or require maintainer credentials during implementation unless explicitly requested.

### CI/release

- [ ] Add release workflow validation for version/tag consistency.
  - Evidence: `tag-release.yml` derives `vX.Y.Z` from `frontend/package.json`; `release.yml` builds any `v*` tag or dispatched tag without checking it matches package version.
  - Acceptance criteria: release jobs fail early if `RELEASE_TAG` does not equal `v$(frontend/package.json version)` or if the version is not strict `X.Y.Z`.
- [ ] Decide whether automatic tag creation on every push to `main` is intended.
  - Evidence: `.github/workflows/tag-release.yml` runs on every `main` push and creates/dispatches a release when the package version tag does not exist.
  - Acceptance criteria: workflow trigger is either documented as intentional release policy or changed to a manual/controlled trigger; no release is published accidentally from routine merges.
- [ ] Verify packaged artifact names and installer expectations across platforms.
  - Evidence: release docs list Windows portable/installer, Linux binary, and macOS zip; `release.yml` publishes portable Windows even if NSIS installer is absent and logs instead of failing.
  - Acceptance criteria: release docs and workflow agree on which artifacts are mandatory vs optional, and missing mandatory artifacts fail release builds.

### Compatibility/operations

- [ ] Add platform-specific path detection tests or fixtures.
  - Evidence: `scanner.DetectAddonPath` hardcodes Windows/OneDrive/macOS/Linux Steam candidates and globs, but tests do not cover path precedence or live/liveeu behavior.
  - Acceptance criteria: path detection logic is made testable with injected home/GOOS/filesystem checks, and tests cover documented candidates without requiring real user directories.
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
