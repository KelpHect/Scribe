# Scribe

Scribe is a fast, Windows-first ESO addon manager written in Rust with Zed's GPUI and `gpui-component`.

It discovers an ESO AddOns folder, fetches the dynamically advertised MMOUI/ESOUI feeds, scans installed manifests, resolves dependencies, and performs staged install/update/uninstall operations with cancellation and rollback.

The native interface is **Scribe Glass**: a dark, acrylic-translucent shell with a 228px icon sidebar, frosted surfaces, and one gold accent, built for mouse and keyboard. Installed, Find More, Updates, and Settings live in the sidebar; each page keeps its actions in a page header row and its filters in a row beneath it. Semantic glass tokens are regression-tested at WCAG 2.x normal-text and non-text indicator thresholds, and the custom Windows title controls expose 46x32 native hit regions. Installed stays grouped by ESOUI category with live category artwork in a virtualized group list; missing thumbnails fall back to that artwork, then to a category-aware product glyph. Find More uses one filter row with fuzzy search (recent searches included) plus batch install, and both lists are fully keyboard-browsable (arrows/`j`/`k`, Enter, `i`, `/`). Large catalogs remain virtualized inside a centered ultrawide-safe surface, while addon rows open dossiers with rich BBCode descriptions and a screenshot rail ahead of the changelog: an 860px modal sheet below 1400px and an inline details page with facts/actions rail on wider windows. Install and update history remains available in the floating Activity surface, with transient status reported as toasts and background update alerts in the system tray. Settings is a single 760px reading column covering the addon library, appearance, notifications, health & recovery, and diagnostics. The current design contract is recorded in [`docs/ui-rework-design.md`](docs/ui-rework-design.md); historical rationale and research remain in [`PRODUCT_DESIGN.md`](PRODUCT_DESIGN.md).

## Build

Rust 1.97 is selected by `rust-toolchain.toml`.

```powershell
cargo build --release --workspace --locked
```

The portable executable is `target/release/scribe.exe`. It is unsigned and currently targets Windows only.

## Install

GitHub Releases ships two Windows artifacts per version:

- `Scribe-<version>-windows-amd64.exe` — the portable executable; run it from anywhere.
- `ScribeSetup-<version>.exe` — an NSIS installer that performs a per-user install (no admin rights) into `%LOCALAPPDATA%\Programs\Scribe`, adds Start Menu shortcuts, and registers an Add/Remove Programs entry. The uninstaller removes only Scribe's program files; `%APPDATA%\Scribe` (settings and `scribe.redb`) is preserved. Close a running Scribe before upgrading.

To rebuild the installer locally, place NSIS 3.x under `target/tools` and run:

```powershell
.\target\tools\nsis-3.10\makensis.exe -DAPP_VERSION=<version> scripts\installer.nsi
```

The script writes `bin\ScribeSetup-<version>.exe` (gitignored) and expects `target\release\scribe.exe` to exist.

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
- `crates/scribe-app`: GPUI windows, persistent page entities, `gpui-component` controls and theming, virtualized lists, task presentation, and Windows integration. The UI is split into `main.rs` (bootstrap, assets, metrics, actions), `theme.rs` (Scribe Glass tokens + dark `ThemeColor`/`ThemeTokens`), `model.rs` (app state, periodic catalog freshness and update alerts), `flows.rs` (actions), `window.rs` (shell and pages), `components.rs` (shared primitives), `rows.rs` (list rows), `overlays.rs` (sheets, lightbox, menus, Activity), `bbcode.rs` (rich descriptions), `tray.rs` (Windows tray icon), and `tests.rs`.
- `assets`: source application assets used by the Rust build.

User settings remain under the historical `Scribe` config directory. The Rust cache is `scribe.redb`; the old `esoui_cache.db` is neither imported nor deleted.

## Background alerts and tray

With **Background update alerts** enabled (default, toggleable in Settings → Notifications), Scribe checks catalog freshness every 15 minutes and refreshes past the 4h TTL. When the update-available count rises, the sidebar Updates badge turns into an accent pill; if the window is minimized, a tray balloon also appears and clicking it opens the Updates page. While enabled, closing the window minimizes it to the system tray instead of quitting — the tray menu offers "Open Scribe", "Check for updates now", and "Quit". When alerts are disabled, closing the window quits Scribe normally and the tray icon is removed.

## Safety

Scribe validates archive paths and addon folder names, stages changes, keeps reversible backups during commit, and confines cleanup/uninstall operations to explicitly named addon folders or Scribe-owned staging directories. Tests never use a real AddOns directory.

Addon metadata and files come from [ESOUI](https://www.esoui.com/) through the [MMOUI API](https://api.mmoui.com/). Scribe has no telemetry, accounts, cloud sync, or alternate addon sources.

The current release executable uses the static MSVC runtime and imports only Windows system DLLs. A clean Windows VM acceptance run and an explicit GPL-compatible distribution decision for GPUI's transitive Zed tracing crates are still required before declaring the portable artifact production-ready.
