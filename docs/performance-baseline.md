# Scribe Performance Baseline

Last baseline refresh: 2026-05-19

This file records the current measured baseline after the SQLite/cache and Installed page indexing pass. Do not treat these numbers as hard pass/fail thresholds yet; use them to compare local changes and decide where optimization is justified.

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
| `BenchmarkScanLargeAddOnsDirectory` | `3,370,432 ns/op`, `6,047,257 B/op`, `31,486 allocs/op` |
| `BenchmarkSQLiteOpenDB` | `959,291 ns/op`, `233,039 B/op`, `3,372 allocs/op` |
| `BenchmarkMatchLargeCatalog` | `1,034,231 ns/op`, `1,402,024 B/op`, `23,037 allocs/op` |
| `BenchmarkCachedCatalogLoad` | `38,204,673 ns/op`, `24,665,221 B/op`, `588,044 allocs/op` |
| `BenchmarkSQLiteSaveRemoteCatalog` | `319,059,876 ns/op`, `17,099,898 B/op`, `211,249 allocs/op` |
| `BenchmarkSQLiteSaveScannerCache` | `6,853,946 ns/op`, `1,366,106 B/op`, `8,317 allocs/op` |
| `BenchmarkSQLiteQueryInstallMD5s` | `1,750,717 ns/op`, `463,935 B/op`, `12,085 allocs/op` |
| `BenchmarkRemoteSearchLargeCatalog` | `490,640 ns/op`, `300,992 B/op`, `13,908 allocs/op` |

SQLite file-size metrics captured during the DB benchmarks:

| Benchmark | DB | WAL | SHM |
| --- | ---: | ---: | ---: |
| `BenchmarkCachedCatalogLoad` | `4,096` | `2,117,712` | `32,768` |
| `BenchmarkSQLiteSaveRemoteCatalog` | `1,998,848` | `6,064,672` | `32,768` |
| `BenchmarkSQLiteSaveScannerCache` | `516,096` | `4,573,232` | `32,768` |
| `BenchmarkSQLiteQueryInstallMD5s` | `114,688` | `4,124,152` | `32,768` |

## Frontend Catalog Benchmarks

| Benchmark | Result |
| --- | ---: |
| `remote search score over large cached catalog` | `878.72 hz`, `1.1380 ms mean`, `2.2006 ms p99` |
| `remote filter metadata preparation` | `308.42 hz`, `3.2424 ms mean`, `3.4461 ms p99` |
| `remote catalog indexed filter/sort` | `1,757.32 hz`, `0.5690 ms mean`, `0.5911 ms p99` |

## Bundle Report Snapshot

Current generated build report:

| Metric | Bytes |
| --- | ---: |
| Total JS | `406,670` |
| Total CSS | `69,115` |
| Total assets | `475,785` |
| Gzip budget | `500,000` |

Largest generated chunks:

| Chunk | Bytes |
| --- | ---: |
| `route-find-more` JS | `245,357` |
| `route-settings` JS | `82,476` |
| `index` CSS | `54,486` |
| `index` JS | `23,618` |
| `route-installed` JS | `20,682` |

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
