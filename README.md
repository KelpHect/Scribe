# Scribe

Scribe is a fast, Windows-first ESO addon manager written in Rust with Zed's GPUI and `gpui-component`.

It discovers an ESO AddOns folder, fetches the dynamically advertised MMOUI/ESOUI feeds, scans installed manifests, resolves dependencies, and performs staged install/update/uninstall operations with cancellation and rollback.

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
```

## Architecture

- `crates/scribe-core`: settings, ESO scanning, MMOUI/ESOUI clients, matching, dependency planning, redb persistence, downloads, installation, rollback, cancellation, and uninstall safety.
- `crates/scribe-app`: GPUI windows, persistent page entities, `gpui-component` controls and theming, virtualized lists, task presentation, and Windows integration.
- `assets`: source application assets used by the Rust build.

User settings remain under the historical `Scribe` config directory. The Rust cache is `scribe.redb`; the old `esoui_cache.db` is neither imported nor deleted.

## Safety

Scribe validates archive paths and addon folder names, stages changes, keeps reversible backups during commit, and confines cleanup/uninstall operations to explicitly named addon folders or Scribe-owned staging directories. Tests never use a real AddOns directory.

Addon metadata and files come from [ESOUI](https://www.esoui.com/) through the [MMOUI API](https://api.mmoui.com/). Scribe has no telemetry, accounts, cloud sync, or alternate addon sources.
