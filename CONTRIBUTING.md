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

## Change boundaries

- Keep install, update, rollback, cancellation, and uninstall safety ahead of micro-optimizations.
- Preserve dynamic MMOUI bootstrap and discovered ESOUI feeds; do not hardcode downstream URLs.
- Keep database access and blocking filesystem/network work off GPUI's render path.
- Use `gpui-component` controls directly where they provide the required keyboard and AccessKit behavior. Scribe-owned elements should be product-specific composition, not a compatibility layer.
- Keep 7,000-item catalogs virtualized and indexed. Do not duplicate complete addon models into filtered views.
- Validate every rkyv archive; never use unchecked archive access.
- Do not modify a real ESO AddOns directory from tests.
- Do not hand-edit generated outputs under `target/` or `bin/`.

## Releases

The root Cargo workspace is the canonical version source. Release, tag, push, and publication operations require an explicit maintainer request. Windows artifacts are currently unsigned and portable-only.
