# Scribe Performance Baseline

Last baseline refresh: 2026-05-18

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

## Go Fixture Benchmarks

| Benchmark | Result |
| --- | ---: |
| `BenchmarkScanLargeAddOnsDirectory` | `2,348,036 ns/op`, `5,745,848 B/op`, `33,434 allocs/op` |
| `BenchmarkMatchLargeCatalog` | `1,194,742 ns/op`, `1,422,838 B/op`, `23,039 allocs/op` |
| `BenchmarkCachedCatalogLoad` | `42,168,042 ns/op`, `24,694,148 B/op`, `588,110 allocs/op` |
| `BenchmarkRemoteSearchLargeCatalog` | `555,851 ns/op`, `300,992 B/op`, `13,908 allocs/op` |

## Frontend Catalog Benchmarks

| Benchmark | Result |
| --- | ---: |
| `remote search score over large cached catalog` | `856.75 hz`, `1.1672 ms mean`, `2.2569 ms p99` |
| `remote filter metadata preparation` | `304.87 hz`, `3.2801 ms mean`, `3.8714 ms p99` |

## Bundle Report Snapshot

Current generated build report:

| Metric | Bytes |
| --- | ---: |
| Total JS | `382,406` |
| Total CSS | `68,684` |
| Total assets | `451,090` |
| Gzip budget | `500,000` |

Largest generated chunks:

| Chunk | Bytes |
| --- | ---: |
| `route-find-more` JS | `231,287` |
| `route-settings` JS | `77,683` |
| `index` CSS | `54,055` |
| `vendor-ui` JS | `20,110` |
| `route-installed` JS | `19,798` |

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
