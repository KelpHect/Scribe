# Contributing

Want to help with Scribe? Nice. Keep it practical.

## Before you open a PR

- open an issue first if the change is big, risky, or changes app behavior in a non-obvious way
- if it's a small fix, just send the PR
- if you're forking it to take the project in a different direction, that's fine too

## What good contributions look like

- small focused diffs
- clear commit messages
- no drive-by refactors unless they are required for the fix
- no plugin API or architecture rewrite work without a separate accepted design plan
- no comment spam. prefer better names and smaller functions
- if you add a comment, explain the constraint or trade-off, not the syntax

## Dev setup

```bash
go install github.com/wailsapp/wails/v2/cmd/wails@v2.12.0
npm --prefix frontend install
wails dev
```

Linux builds also need native Wails dependencies. Install the distro packages before `wails dev` or `wails build`.

Debian/Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config npm libgtk-3-dev libwebkit2gtk-4.1-dev
```

Fedora:

```bash
sudo dnf upgrade
sudo dnf install -y gcc-c++ pkgconf-pkg-config npm gtk3-devel webkit2gtk4.1-devel
```

## Before you submit

```bash
./scripts/verify.sh
```

This runs diff sanity, regenerates Wails bindings and `frontend/dist`, then runs frontend type checks, frontend smoke tests, and Go tests. Generated files are build output; do not hand-edit them.

Use these focused checks while you work:

```bash
npm --prefix frontend run check
npm --prefix frontend run test
npm --prefix frontend run lint:check
npm --prefix frontend run build
go test ./...
wails build -tags webkit2_41   # Linux
wails build                    # Windows/macOS
```

Run `wails build` before root `go test ./...` on a clean checkout so `frontend/dist/` exists. Use `npm --prefix frontend run lint:check` for lint verification. Avoid `npm --prefix frontend run lint` unless you want ESLint autofixes applied.

## Generated files

Wails generates `frontend/wailsjs/` and `frontend/dist/`.

- recover missing or stale bindings with `wails dev` or `wails build`
- recover missing embedded frontend assets with `wails build`
- do not hand-edit generated files
- run `./scripts/verify.sh` from a clean checkout when you want the full recovery-and-check path

## Local data reset

The app database is `Scribe/esoui_cache.db` under the OS user config directory. It stores settings, ESOUI cache rows, search presets, and install MD5 records. It does not contain addon files.

For local troubleshooting, close Scribe and rename or delete only that database file:

- Windows: `%AppData%\Scribe\esoui_cache.db`
- macOS: `~/Library/Application Support/Scribe/esoui_cache.db`
- Linux: `~/.config/Scribe/esoui_cache.db`

Do not reset a real ESO `AddOns` folder unless the task explicitly calls for addon install/update/uninstall behavior.

## Performance checks

Scribe's diagnostics panel in Settings reports startup and memory metrics. For PRs that touch startup, route loading, caching, background refresh, or long-lived stores, include a diagnostics snapshot after a fresh app launch.

Capture at least:

- frontend-ready time, target under 1000 ms
- Go `Sys` memory, target at or under 150 MB
- remote addon/category counts and cache-stale state
- remote refresh count and last refresh duration
- OS, app version or commit, and whether the catalog was warm or cold

Threshold changes above those targets need maintainer approval in the PR or issue. Prefer measuring before optimizing; do not trade correctness or AddOns safety for a lower number.

For repeatable fixture benchmarks, run:

```bash
./scripts/benchmarks.sh
```

This covers large AddOns scanning, cached catalog load, backend remote search, frontend catalog filtering/ranking, and the documented cold/warm startup diagnostics capture path. Treat these fixture numbers as baselines to record before enforcement, not as release-blocking thresholds.

For backend hot-path profiling, run:

```bash
./scripts/profile-backend.sh
```

It captures CPU and memory profiles for scanner scans, cached catalog load, matching/search, and dependency resolution under `build/reports/profiles/`. The output is ignored by git; summarize the top costs in the PR or issue before optimizing.

For local profiling, start the app with `SCRIBE_PPROF=1` to expose pprof on `localhost:6060`. The old `SCRIBEEGO_PPROF=1` spelling still works for compatibility.

If you touch release workflows or packaging, say that clearly in the PR body.

Release tagging is manual. The tag-release workflow reads `frontend/package.json`, creates `vX.Y.Z`, and dispatches the release only when a maintainer runs it.

Release builds require the Windows portable exe, Linux binary, and macOS universal zip. The Windows NSIS installer is optional and is uploaded only if Wails produces it.

Strong Windows signing and macOS notarization are not part of the current release flow. Do not add signing/notarization automation unless maintainer certificates, Apple credentials, secret handling, and release approval are already defined.

## Style

- write code like a maintainer has to live with it for a year
- prefer the smallest correct change
- avoid filler comments
- keep docs direct and honest

## Issues

Good bug reports include:

- what you tried
- what happened
- what you expected
- OS and app version
- screenshots or logs if the UI broke

## PRs

- explain the user-visible change first
- call out trade-offs and follow-up work
- keep AI-looking boilerplate out of the description

That's it. Make it easier to maintain, not harder.
