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

## Releases

- `Scribe-windows-amd64.exe`
- `Scribe-windows-amd64-installer.exe`
- `Scribe-linux-amd64`
- `Scribe-macos-universal.zip`

## What it does well

- browse addon pages without dumping ESOUI's weird formatting straight into your face
- install or update addons with queueing and progress
- install missing required dependencies in one click
- install optional dependencies too if you want the extra features
- keep installed addons grouped and searchable without losing your UI state every launch

## Known limitations

- Windows builds are unsigned, so SmartScreen may complain
- Linux builds use UPX in CI
- macOS builds are ad-hoc signed, not notarized, so Gatekeeper may still warn
- addon metadata comes from ESOUI/MMOUI, so upstream version weirdness still leaks through sometimes

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

### stack

- Go 1.23
- Wails v2
- Svelte 5 + TypeScript
- Tailwind CSS v4
- Vite 8
- SQLite via GORM + `glebarez/sqlite`

### gotchas for contributors

- this is a Wails app, not SvelteKit
- `frontend/wailsjs/` is generated
- release automation reads the version from `frontend/package.json`

</details>
