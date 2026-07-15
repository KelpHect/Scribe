# AGENTS.md

## Scope

- The active Scribe product is the Windows-first Rust workspace using GPUI and `gpui-component`.
- Do not commit, push, tag, publish, dispatch workflows, or create releases unless the user explicitly requests that exact operation.
- Do not add alternate addon sources, accounts, cloud sync, telemetry, plugin APIs, signing, or notarization without an accepted plan.
- Never delete, move, or bulk-modify user addon folders beyond the explicitly named install/update/uninstall action.

## Stack

- Rust 1.97, edition 2024, committed `Cargo.lock`.
- One lockfile-pinned current Zed GPUI graph with `gpui-component`; no Guise or compatibility fork.
- Raw redb 4.1, Postcard for owned/versioned records, and validated rkyv 0.8 archives for measured catalog access.
- Windows is the only current product target. The portable executable is unsigned.

## Completion gates

Run for Rust changes:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo build --release --workspace --locked
cargo deny check
git diff --check
```

Run the relevant release-mode fixture for performance-sensitive changes.

## Architecture

- `scribe-core` owns settings, scanning/parsing, MMOUI/ESOUI access, catalog/indexing, matching, dependency planning, persistence, downloads, installation, rollback, cleanup, cancellation, and uninstall safety.
- `scribe-app` owns GPUI entities/actions/windows, `gpui-component` presentation, persistent navigation state, virtual lists, accessibility, task UI, and Windows integration.
- Keep filesystem, network, redb transactions, `AccessGuard`, and archive ownership out of GPUI rendering.
- Keep `Catalog` and filtered views index-based; avoid repeated normalization, parsing, sorting, or model cloning in hot render paths.
- Use direct component APIs and theme tokens. Scribe-owned components are allowed for product-specific virtual rows or a documented accessibility gap, not to preserve an obsolete UI abstraction.

## Persistence and network

- Preserve the historical config directory name `Scribe` and `settings.toml`.
- Rust data lives in `scribe.redb`. Leave legacy `esoui_cache.db` untouched.
- Never overwrite an unreadable database. Retain it and require an explicit rebuild action.
- Preserve the four-hour catalog TTL, normalized hash, and dynamic `https://api.mmoui.com/v3/globalconfig.json` bootstrap.
- Preserve 30-second requests and three cancellation-aware transport/5xx attempts with 1/2/4-second backoff.
- MMOUI uses explicit JSON `null` for list-shaped fields such as siblings, screenshots, compatibility, and category parents. Deserialize those as empty lists and keep a live-payload-shaped regression fixture.
- Validate rkyv with bytecheck and lifetime-safe ownership. Never call `access_unchecked`.
- Cached catalogs use the validated rkyv codec marker and archive-backed `CatalogIndex`; old Postcard catalog blobs are reconstructible misses, while scanner/install records remain owned Postcard data.

## UI and performance

- Every window uses `gpui_component::Root` and the component `TitleBar`.
- Settings are an integrated persistent page; do not reintroduce a duplicate settings window or a second Scribe wordmark in the sidebar.
- Follow compact dark chrome, warm parchment work surfaces, dense metadata, and utility-first desktop interactions. Do not replace this with generic white/dark component-library defaults.
- Render dialogs and screenshot lightboxes as final children of the window root so their backdrop covers the title bar, sidebar, toolbar, and content. Modal children must stop pointer propagation to the catalog beneath them.
- Installed addons stay category-grouped with MMOUI category icons, collapsible headers, dependency banners, and row-click details. Find More keeps search, category, content, compatibility, sort, direction, hide-installed, and refresh controls together in its toolbar.
- Addon rows open details on click or Enter/Space; action buttons stop propagation. Remote details place the bounded screenshot rail before description/changelog, and screenshots open a full-window previous/next lightbox.
- Keep large catalogs virtualized with stable row geometry and bounded overscan.
- Keep the precomputed popularity order in `CatalogIndex`; empty and filtered catalog views should not sort all addons in GPUI rendering.
- Apply Scribe's custom theme to both `ThemeColor` and `ThemeTokens`; the component title bar reads tokens while most controls read colors.
- Keep remote images fixed-size, lazy, failure-safe, and bounded in detail views.
- Task state transitions are immediate; repeated byte/file counters remain throttled.
- Measure startup, useful-window latency, RSS, list filtering/sorting, scroll behavior, and archive-ready latency before accepting performance claims.
- Prefer removing startup work, lazy initialization, bounded caching, and direct native/GPUI APIs over compatibility layers or speculative dependencies.

## Safety

- Reject traversal, absolute paths, separators, dot names, symlink escapes, and malformed ZIP entries.
- Preserve MD5 integrity checks, staging, backup, atomic rename commit, reverse rollback, cancellation, and Scribe-owned temporary cleanup.
- Required and optional dependencies stay separate and resolve to the latest canonical ESOUI entry.
- Tests use temporary folders/databases and mocked MMOUI fixtures, never user data.

## Documentation and generated files

- Keep `README.md` and `CONTRIBUTING.md` synchronized with architecture, checks, persistence, packaging, and measured baselines.
- Treat `target/`, `bin/`, benchmark/profile reports, and packaged executables as generated.

## Blockers

Stop and ask if work needs real AddOns data, credentials, publishing authority beyond an explicit request, a new remote source, signing material, or a destructive action outside a named addon operation.
