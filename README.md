# Scribe

Scribe is a fast, Windows-first ESO addon manager written in Rust with Zed's GPUI and `gpui-component`.

It discovers an ESO AddOns folder, fetches the dynamically advertised MMOUI/ESOUI feeds, scans installed manifests, resolves dependencies, and performs staged install/update/uninstall operations with cancellation and rollback.

The native interface is **Scribe Glass**: a dark, acrylic-translucent shell with a 228px icon sidebar, frosted surfaces, and one gold accent, built for mouse and keyboard. Installed, Find More, Updates, and Settings live in the sidebar; each page keeps its actions in a page header row and its filters in a row beneath it. Semantic glass tokens are regression-tested at WCAG 2.x normal-text and non-text indicator thresholds, and the custom Windows title controls expose 46x32 native hit regions. Installed stays grouped by ESOUI category with live category artwork; missing thumbnails fall back to that artwork, then to a category-aware product glyph. Find More uses one filter row for search, category, compatibility, sort, and installed visibility. Large catalogs remain virtualized inside a centered ultrawide-safe surface, while addon rows open dossiers with the screenshot rail ahead of description and changelog: a centered 860px sheet at compact sizes and a bounded right-hand sheet on wide windows. Install and update history remains available in the floating Activity surface until dismissed. Settings is a single 760px reading column covering the addon library, appearance, health & recovery, and diagnostics. The current design contract is recorded in [`docs/ui-rework-design.md`](docs/ui-rework-design.md); historical rationale and research remain in [`PRODUCT_DESIGN.md`](PRODUCT_DESIGN.md).

## Build

Rust 1.97 is selected by `rust-toolchain.toml`.

```powershell
cargo build --release --workspace --locked
```

The portable executable is `target/release/scribe.exe`. It is unsigned and currently targets Windows only.

## Verify

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo build --release --workspace --locked
cargo deny check
```

Performance fixtures:

```powershell
cargo run --release -p scribe-core --example storage_gate --locked
cargo run --release -p scribe-core --example codec_gate --features rkyv-bench --locked
cargo run --release -p scribe-core --example scanner_gate --locked
cargo run --release -p scribe-core --example installer_gate --locked
.\scripts\measure-startup.ps1 -Runs 10 -PrimeDelayMs 10000
.\scripts\acceptance-windows.ps1 -Executable target\release\scribe.exe
```

The acceptance script uses a disposable `APPDATA`/`LOCALAPPDATA` profile. It verifies embedded assets, first-frame and catalog-ready markers, redb creation, graceful close, restart, and settings-file stability without reading or changing a real AddOns folder.

## Architecture

- `crates/scribe-core`: settings, ESO scanning, MMOUI/ESOUI clients, matching, dependency planning, redb persistence, downloads, installation, rollback, cancellation, and uninstall safety.
- `crates/scribe-app`: GPUI windows, persistent page entities, `gpui-component` controls and theming, virtualized lists, task presentation, and Windows integration. The UI is split into `main.rs` (bootstrap, assets, metrics, actions), `theme.rs` (Scribe Glass tokens + dark `ThemeColor`/`ThemeTokens`), `model.rs` (app state), `flows.rs` (actions), `window.rs` (shell and pages), `components.rs` (shared primitives), `rows.rs` (list rows), `overlays.rs` (sheets, lightbox, menus, Activity), and `tests.rs`.
- `assets`: source application assets used by the Rust build.

User settings remain under the historical `Scribe` config directory. The Rust cache is `scribe.redb`; the old `esoui_cache.db` is neither imported nor deleted.

## Safety

Scribe validates archive paths and addon folder names, stages changes, keeps reversible backups during commit, and confines cleanup/uninstall operations to explicitly named addon folders or Scribe-owned staging directories. Tests never use a real AddOns directory.

Addon metadata and files come from [ESOUI](https://www.esoui.com/) through the [MMOUI API](https://api.mmoui.com/). Scribe has no telemetry, accounts, cloud sync, or alternate addon sources.

The current release executable uses the static MSVC runtime and imports only Windows system DLLs. A clean Windows VM acceptance run and an explicit GPL-compatible distribution decision for GPUI's transitive Zed tracing crates are still required before declaring the portable artifact production-ready.
