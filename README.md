# Scribe

Scribe is an ESO addon manager for people who are tired of fighting Minion.

It scans your AddOns folder, pulls addon data from ESOUI/MMOUI, handles installs and updates, and stays quick even when your addon pile gets silly.

## Why this exists

Managing ESO addons shouldn't mean waiting on a bloated app, guessing which library is missing, or staring at broken update badges.

Scribe is built to handle the annoying parts cleanly:

- find your AddOns folder
- show what's installed
- browse ESOUI
- install and update addons with progress
- one-click install missing required and optional dependencies
- catch missing dependencies before the game does

## Quick start

```bash
go install github.com/wailsapp/wails/v2/cmd/wails@v2.12.0
git clone https://github.com/KelpHect/Scribe.git
cd Scribe
wails build
```

Release builds end up in `build/bin/`.

### Linux build packages

Linux Wails builds need GTK, WebKitGTK, a C/C++ compiler, `pkg-config`, and npm. Scribe builds Linux with Wails' `webkit2_41` tag.

Debian/Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config npm libgtk-3-dev libwebkit2gtk-4.1-dev
wails build -tags webkit2_41
```

Fedora:

```bash
sudo dnf upgrade
sudo dnf install -y gcc-c++ pkgconf-pkg-config npm gtk3-devel webkit2gtk4.1-devel
wails build -tags webkit2_41
```

## Releases

Mandatory release assets:

- `Scribe-windows-amd64.exe`
- `Scribe-linux-amd64`
- `Scribe-macos-universal.zip`

Optional release asset:

- `Scribe-windows-amd64-installer.exe` when the NSIS build produces an installer

## What it does well

- browse addon pages without dumping ESOUI's weird formatting straight into your face
- show cached ESOUI catalog data clearly when background refreshes fail
- install or update addons with queueing and progress
- avoid duplicate install/update queue entries from rapid repeated actions
- install missing required dependencies in one click
- install optional dependencies too if you want the extra features
- keep destructive uninstall and folder-opening actions constrained to the configured AddOns directory
- keep installed addons grouped and searchable without losing your UI state every launch

## Known limitations

- Windows builds are unsigned, so SmartScreen may complain
- Linux builds use UPX in CI
- macOS builds are ad-hoc signed, not notarized, so Gatekeeper may still warn
- addon metadata comes from ESOUI/MMOUI, so upstream version weirdness still leaks through sometimes

## Local data

Scribe stores user-facing settings in `settings.toml` and keeps ESOUI catalog cache, search presets, scanner cache, and install MD5 records in `esoui_cache.db`. Both files live under the OS user config directory in a `Scribe` folder.

- Windows: `%AppData%\Scribe\settings.toml` and `%AppData%\Scribe\esoui_cache.db`
- macOS: `~/Library/Application Support/Scribe/settings.toml` and `~/Library/Application Support/Scribe/esoui_cache.db`
- Linux: `~/.config/Scribe/settings.toml` and `~/.config/Scribe/esoui_cache.db`

Those files are separate from your ESO `AddOns` folder. To reset Scribe's settings or cache, close Scribe and rename or delete only `settings.toml` and/or `esoui_cache.db`; do not delete your AddOns directory. Scribe will recreate missing files on next launch and refresh ESOUI data.

Scribe does not include telemetry or analytics. Network requests are for MMOUI/ESOUI catalog/download behavior and user-triggered external links.

## Performance budgets

Settings includes a diagnostics panel for startup, memory, catalog, and remote-refresh metrics. Startup-related PRs should include a fresh-launch diagnostics snapshot. Current targets are frontend-ready under 1000 ms and Go `Sys` memory at or under 150 MB.

Run repeatable local fixtures with:

```bash
./scripts/benchmarks.sh
```

The script covers large AddOns scans, cached catalog load, backend remote search, frontend catalog filtering/ranking, and prints the cold/warm startup plus memory snapshot capture steps. Fixture benchmark numbers are recorded before enforcement; only the diagnostics budgets above are current targets.

Backend hot-path profiles can be captured with `./scripts/profile-backend.sh`. It writes CPU and memory profiles for scanner scans, cached catalog load, matching/search, and dependency resolution under `build/reports/profiles/`, which is ignored by git.

Frontend workflow smoke/profile reports can be captured with:

```bash
./scripts/profile-ui-workflows.sh
```

The script runs fixture-backed frontend workflow tests, catalog benchmarks, and a production frontend build, then writes an ignored report under `build/reports/ui-profile/`. It is not a replacement for manually launching the Wails app before release, but it gives a repeatable local signal for Installed, Find More, Updates, Settings, addon detail data, dependency banners, task center, and failure/retry states.

### Performance coding patterns

- Keep startup work background-first: render from cached state, then refresh scans/catalog data asynchronously.
- Keep Find More search/filter/sort logic in tested indexed helpers instead of rebuilding lowercase/search/version work in route markup.
- Keep large lists virtualized with stable row dimensions, fixed image boxes, lazy image loading, async decoding, and bounded overscan.
- Coalesce high-frequency Wails bridge progress events before reactive store writes; task state transitions stay immediate, byte/file counters can be batched.
- Keep install/update presentation shared through small helpers for preflight, rollback language, update reasons, dependency display, and failure recovery.
- Keep user settings in `settings.toml`, cache/state records in SQLite, and generated outputs out of hand-written changes.

## When not to use this

- you want a signed and notarized app on every platform right now
- you need addon sources outside ESOUI/MMOUI
- you want cloud sync, accounts, or plugin APIs. this app is intentionally small

## Credits

Addon listings, metadata, thumbnails, category icons, and download files come from [ESOUI.com](https://www.esoui.com/) through the public [MMOUI API](https://api.mmoui.com/).

Addon files stay hosted by ESOUI and belong to their respective authors. Scribe does not mirror or redistribute them.

_The Elder Scrolls Online_ is a registered trademark of ZeniMax Media Inc. Scribe is an independent community project and is not affiliated with ZeniMax Media, Bethesda Softworks, or ESOUI.com.

## Contributing

Want to help? Open an issue, send a PR, or fork it and go wild. The short version lives in [CONTRIBUTING.md](CONTRIBUTING.md).

<details>
<summary>dev notes</summary>

### local dev

```bash
wails dev
```

### checks

```bash
./scripts/verify.sh
./scripts/benchmarks.sh
./scripts/profile-backend.sh
npm --prefix frontend run check
npm --prefix frontend run test
npm --prefix frontend run bench -- --run
npm --prefix frontend run lint:check
npm --prefix frontend run build
go test ./...
```

On Linux, local Wails builds need `-tags webkit2_41` after installing the GTK/WebKit packages above. Use `npm --prefix frontend run lint:check` for lint verification; `npm --prefix frontend run lint` applies autofixes.

### generated files

`frontend/wailsjs/` and `frontend/dist/` are generated by Wails. Do not hand-edit them or treat them as source files.

If `frontend/wailsjs/` is missing or stale, run `wails dev` or `wails build` to regenerate bindings. If `frontend/dist/` is missing, run `wails build`; root `go test ./...` needs those embedded assets to exist. From a clean checkout, `./scripts/verify.sh` does the recovery path for you.

### stack

- Go 1.26.3
- Node.js 24 + npm 11
- Wails v2
- Svelte 5 + TypeScript
- Tailwind CSS v4
- Vite 8
- SQLite via GORM + `glebarez/sqlite`

### gotchas for contributors

- this is a Wails app, not SvelteKit
- `frontend/wailsjs/` and `frontend/dist/` are generated by Wails
- run `./scripts/verify.sh` from a clean checkout to run diff sanity, regenerate generated files, type-check/test the frontend, and run Go tests
- set `SCRIBE_PPROF=1` to start the local pprof server on `localhost:6060`; `SCRIBEEGO_PPROF=1` is kept as a legacy alias
- release tagging is manual; the tag-release workflow reads the version from `frontend/package.json`

</details>
