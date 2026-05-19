# Scribe Performance Baseline

Last baseline refresh: 2026-05-19

This file records the current measured baseline before additional P9 performance work. Do not treat these numbers as hard pass/fail thresholds yet; use them to compare local changes and decide where optimization is justified.

## Environment

- OS/runtime: Linux amd64
- CPU reported by Go benchmarks: 13th Gen Intel(R) Core(TM) i7-13700KF
- Verification context: fixture-backed benchmarks only; no real ESO AddOns directory and no live ESOUI dependency.

## Commands

```bash
./scripts/benchmarks.sh
```

The full verification gate for the accompanying task is:

```bash
./scripts/verify.sh
```

Real desktop captures use the opt-in pprof server:

```bash
SCRIBE_PPROF=1 ./build/bin/Scribe &
./scripts/profile-desktop.sh
```

Use the resulting `build/reports/desktop-profile/desktop.cpu.pprof` as optional `SCRIBE_PGO_PROFILE` input only after the profile covers a representative Scribe session.

## Go Fixture Benchmarks

| Benchmark | Result |
| --- | ---: |
| `BenchmarkScanLargeAddOnsDirectory` | `3,549,085 ns/op`, `6,028,464 B/op`, `31,483 allocs/op` |
| `BenchmarkMatchLargeCatalog` | `1,782,351 ns/op`, `1,425,139 B/op`, `23,039 allocs/op` |
| `BenchmarkCachedCatalogLoad` | `41,588,164 ns/op`, `24,694,533 B/op`, `588,112 allocs/op` |
| `BenchmarkRemoteSearchLargeCatalog` | `593,538 ns/op`, `300,992 B/op`, `13,908 allocs/op` |

## Frontend Catalog Benchmarks

| Benchmark | Result |
| --- | ---: |
| `remote search score over large cached catalog` | `880.23 hz`, `1.1361 ms mean`, `2.2420 ms p99` |
| `remote filter metadata preparation` | `301.88 hz`, `3.3125 ms mean`, `3.7690 ms p99` |
| `remote catalog indexed filter/sort` | `1,750.80 hz`, `0.5712 ms mean`, `0.8982 ms p99` |

## Bundle Report Snapshot

Current generated build report:

| Metric | Bytes |
| --- | ---: |
| Total JS | `405,706` |
| Total CSS | `70,595` |
| Total assets | `476,301` |
| Gzip budget | `500,000` |

Largest generated chunks:

| Chunk | Bytes |
| --- | ---: |
| `route-find-more` JS | `244,715` |
| `route-settings` JS | `82,404` |
| `index` CSS | `55,966` |
| `index` JS | `23,623` |
| `route-installed` JS | `20,427` |

## Interactive Diagnostics Capture

Cold and warm startup diagnostics require launching the real desktop app because Wails startup timings and WebKitGTK behavior are not represented by fixture tests.

Capture procedure:

1. Build or launch the app with the normal local command.
2. For a cold run, quit Scribe and temporarily move the local `Scribe/esoui_cache.db` out of the config directory.
3. Launch Scribe, wait until the UI is idle, open Settings diagnostics, and copy the diagnostics export.
4. Restore or keep the cache, relaunch Scribe, wait until idle, and copy the warm diagnostics export.
5. Record `startupMs`, `frontendReadyMs`, `remoteReadyMs`, `heapAllocMb`, `sysMb`, remote count, installed count, and persistence status.

Manual Find More profile:

1. Open Find More with a populated cached catalog.
2. Type a common addon query and record visible typing delay.
3. Clear search, change category/version/content filters, and record visible delay.
4. Scroll from top to bottom and back; note blank rows, image shifts, dropped frames, or delayed row actions.
5. Open and close a remote addon details dialog and note image/detail load delay.

These interactive values should be appended here once captured on a real desktop session.
