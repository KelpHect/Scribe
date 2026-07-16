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

## UI source layout (scribe-app)

- `main.rs` — bootstrap only: `main()`, `ScribeAssets`, `LazyHttpClient`, UI metrics/startup trace, actions + keybindings, reduced-motion detection, module declarations.
- `theme.rs` — the single source of design tokens: every Scribe Glass color/surface/radius/spacing/motion constant plus `apply_scribe_theme`. Never hardcode hex in another module.
- `model.rs` — `AppModel`, `Page`, `OverlayKind`, `PageState`, health/status types, navigation and page-state replacement helpers.
- `flows.rs` — app-level actions: install/update queues, details loading, uninstall, rebuild, theme/folder pickers, refresh/rescan, MD5 enrichment.
- `window.rs` — `ScribeWindow` (state, subscriptions, focus/overlay plumbing), the sidebar shell, page header/filter rows, all four pages, `Render`.
- `components.rs` — shared primitives: `NativeButton`/`NativeIconButton`, `Modal`, `LiveRegion`, filter controls, category picker overlay, notices, cards, artwork helpers.
- `rows.rs` — catalog and installed row renderers.
- `overlays.rs` — details sheets, lightbox, confirm modals, dependency banners, context menus, the Activity surface.
- `tests.rs` — all tests (updated, never deleted, when presentation changes).

## Presentation (Scribe Glass)

The design source of truth is `docs/ui-rework-design.md`. Key rules:

- The window uses `gpui_component::Root` plus `WindowBackgroundAppearance::Blurred` (acrylic). GPUI then clears the framebuffer transparent, so the app MUST paint its own background: the content column paints `SCRIBE_WINDOW_TINT_RGBA` and the sidebar paints `SCRIBE_SIDEBAR_TINT_RGBA`, each as a single layer. Never leave the window root unpainted and never stack a second alpha tint over already-tinted chrome (double compositing shows seams).
- Theme is dark-first: `apply_scribe_theme` runs `ThemeMode::Dark` and writes BOTH `ThemeColor` and regenerated `ThemeTokens` so gpui-component widgets (Input, PopupMenu, Checkbox, scrollbars, dropdowns) match the glass palette. Root font is `.Segoe UI Variable Text`; honor the existing reduced-motion wiring for all animation. Semantic color pairs keep WCAG contrast regression coverage in `tests.rs`.
- Shell: 228px left rail (sidebar tint + hairline) with the single Scribe wordmark, icon nav with accent selection and count badges, Settings pinned to the bottom. Content column: 52px title row, then a page header row (per-page actions), then the filter row, then an inline status banner. Work surfaces are centered at 1200px (Settings: 760px reading column) with 28px gutters.
- The component `TitleBar` stays (it owns platform title-bar behavior) but is transparent; the Scribe Windows-control overlay stays above it with 46x32 native `WindowControlArea` hit regions. The TitleBar is extended past the window's right edge inside a clipping wrapper so the upstream control glyphs park off-screen — do not paint an occluding block over the strip.
- Buttons are pills; cards are rounded-14 `SURFACE` + `HAIRLINE`; sheets/popovers/menus use `SURFACE_RAISED` + `HAIRLINE` (radius 18 sheets, 12 menus/palette); focus rings use the accent ring token.
- Never call `.hover()`/`.focus()` style closures on gpui-component widgets (`Button`, `Select`, `Input`, etc.) — they set their own internal hover styles and panic ("hover style already set"). Style closures belong on plain gpui `div`s only.
- Render dialogs, sheets, and screenshot lightboxes as final children of the window root so their backdrop covers the title bar, sidebar, and content; modal children must stop pointer propagation to the catalog beneath them.
- Pages: Installed stays category-grouped (MMOUI category artwork, collapsible headers, dependency banners, explicit bulk-selection mode with selection bar). Find More has one searchable icon-bearing category picker, All/Latest compatibility, hybrid sort/direction, hide-installed, refresh, and search controls; do not add parallel category selectors. Settings is one scrolling column of four cards (library, appearance, health & recovery, about & diagnostics); do not reintroduce the index rail or a second wordmark.
- Missing addon thumbnails use MMOUI category artwork first and Scribe's category-aware semantic fallback second; do not use generic letters or an unrelated stock placeholder.
- Addon rows open details on click or Enter/Space; action buttons stop propagation. Remote details place the bounded screenshot rail before description/changelog, and screenshots open a full-window previous/next lightbox.
- Status notices stay a slim inline banner with `LiveRegion` semantics; task progress stays in the floating Activity surface (collapsed pill + expanded panel). Do not dock a task center into page content or duplicate task failures in both the global status surface and Activity.

## UI and performance

- Keep large catalogs virtualized with stable row geometry (72px catalog pitch) and bounded overscan.
- Keep the precomputed popularity order in `CatalogIndex`; empty and filtered catalog views should not sort all addons in GPUI rendering.
- Keep remote images fixed-size, lazy, failure-safe, and bounded in detail views.
- Task state transitions are immediate; repeated byte/file counters remain throttled.
- Measure startup, useful-window latency, RSS, list filtering/sorting, scroll behavior, and archive-ready latency before accepting performance claims.
- Prefer removing startup work, lazy initialization, bounded caching, and direct native/GPUI APIs over compatibility layers or speculative dependencies.

## Persistence and network

- Preserve the historical config directory name `Scribe` and `settings.toml`.
- Rust data lives in `scribe.redb`. Leave legacy `esoui_cache.db` untouched.
- Never overwrite an unreadable database. Retain it and require an explicit rebuild action.
- Preserve the four-hour catalog TTL, normalized hash, and dynamic `https://api.mmoui.com/v3/globalconfig.json` bootstrap.
- Preserve 30-second requests and three cancellation-aware transport/5xx attempts with 1/2/4-second backoff.
- MMOUI uses explicit JSON `null` for list-shaped fields such as siblings, screenshots, compatibility, and category parents. Deserialize those as empty lists and keep a live-payload-shaped regression fixture.
- Validate rkyv with bytecheck and lifetime-safe ownership. Never call `access_unchecked`.
- Cached catalogs use the validated rkyv codec marker and archive-backed `CatalogIndex`; old Postcard catalog blobs are reconstructible misses, while scanner/install records remain owned Postcard data.

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
