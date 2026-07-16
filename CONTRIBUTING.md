# Contributing

Scribe's active application is the Rust workspace at the repository root.

## Setup and checks

Install the Rust toolchain selected by `rust-toolchain.toml`, then run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo build --release --workspace --locked
cargo deny check
```

Use the relevant release-mode example under `crates/scribe-core/examples` for performance-sensitive storage, catalog, scanner, or installer changes. Record before/after medians on the same fixture and machine.

Run `scripts/acceptance-windows.ps1` against the release executable for first-launch/restart work. The harness is confined to a disposable profile and must pass on a clean Windows VM before a portable artifact is described as production-ready.

## Change boundaries

- Keep install, update, rollback, cancellation, and uninstall safety ahead of micro-optimizations.
- Preserve dynamic MMOUI bootstrap and discovered ESOUI feeds; do not hardcode downstream URLs.
- Keep database access and blocking filesystem/network work off GPUI's render path.
- Use `gpui-component` controls directly where they provide the required keyboard and AccessKit behavior. Scribe-owned elements should be product-specific composition, not a compatibility layer.
- Keep dialogs and lightboxes as final window-root children, trap and restore focus, name every interactive control, and verify the real Windows AccessKit tree rather than relying only on source inspection.
- Use Scribe's glass tokens from `theme.rs` for foreground/background/status/focus pairs; never hardcode hex in another module. Normal text pairs must meet WCAG AA contrast, and focus indicators must remain visible on the dark glass surfaces. The window background is acrylic: app surfaces paint their own alpha tints, always as a single layer over the window tint.
- Preserve the Scribe Glass shell hierarchy documented in `docs/ui-rework-design.md`: the 228px sidebar with the single wordmark, per-page header and filter rows inside the content column, the centered 1200px work surface (760px Settings column), rounded-14 cards, `SURFACE_RAISED` overlays rendered as final window-root children, actionable-only inline notices, and the floating Activity surface. Do not restore a horizontal command deck, a second wordmark, persistent status strips, or docked task panels.
- Never attach `.hover()`/`.focus()` style closures to gpui-component widgets (Button, Select, Input, etc.); they own their internal hover styles and panic on duplicates. Use plain gpui `div`s for custom interactive styling.
- Keep one obvious route for each discovery task: Find More uses a single searchable MMOUI category picker, a two-state All/Latest compatibility control, and one hybrid sort/direction control. Do not duplicate category choices as shortcut chips, an inline atlas, and a select.
- Missing addon thumbnails fall back to their MMOUI category artwork. If that remote artwork is absent or fails, use Scribe's category-aware semantic fallback; generic letters and unrelated component-library placeholders are not product artwork.
- Respect GPUI's central reduced-motion setting. New motion must be brief, purposeful, and able to resolve immediately to its final state.
- Keep 7,000-item catalogs virtualized and indexed. Do not duplicate complete addon models into filtered views.
- Validate every rkyv archive; never use unchecked archive access.
- Do not modify a real ESO AddOns directory from tests.
- Do not hand-edit generated outputs under `target/` or `bin/`.

## Releases

The root Cargo workspace is the canonical version source. Release, tag, push, and publication operations require an explicit maintainer request. Windows artifacts are currently unsigned and portable-only.
