# Scribe Performance Baseline

Last baseline refresh: 2026-05-19

This file records the current measured baseline after the SQLite custom-binary snapshot/hash/diff cache pass and Installed page indexing pass. Do not treat these numbers as hard pass/fail thresholds yet; use them to compare local changes and decide where optimization is justified.

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
| `BenchmarkScanLargeAddOnsDirectory` | `3,890,009 ns/op`, `6,061,742 B/op`, `31,488 allocs/op` |
| `BenchmarkSQLiteOpenDB` | `1,097,164 ns/op`, `270,075 B/op`, `3,907 allocs/op` |
| `BenchmarkMatchLargeCatalog` | `1,054,216 ns/op`, `1,401,816 B/op`, `23,037 allocs/op` |
| `BenchmarkCachedCatalogLoad` | `2,812,270 ns/op`, `6,096,616 B/op`, `105,105 allocs/op` |
| `BenchmarkSQLiteSaveRemoteCatalog` unchanged | `10,710,884 ns/op`, `10,497,577 B/op`, `98,097 allocs/op` |
| `BenchmarkSQLiteSaveRemoteCatalogChangedOne` | `28,093,800 ns/op`, `22,639,804 B/op`, `385,004 allocs/op` |
| `BenchmarkSQLiteSaveRemoteCatalogInitial` | `150,900,757 ns/op`, `25,781,408 B/op`, `258,959 allocs/op` |
| `BenchmarkSQLiteSaveScannerCache` | `5,425,126 ns/op`, `1,365,996 B/op`, `8,316 allocs/op` |
| `BenchmarkSQLiteQueryInstallMD5s` | `1,695,380 ns/op`, `463,908 B/op`, `12,085 allocs/op` |
| `BenchmarkRemoteSearchLargeCatalog` | `485,992 ns/op`, `300,992 B/op`, `13,908 allocs/op` |

Snapshot codec benchmark for the 7k-addon fixture:

| Benchmark | Result |
| --- | ---: |
| `BenchmarkRemoteCatalogSnapshotCodecs/JSONDecode` | `20,736,466 ns/op`, `9,569,073 B/op`, `105,025 allocs/op` |
| `BenchmarkRemoteCatalogSnapshotCodecs/GobDecode` | `4,027,554 ns/op`, `6,140,640 B/op`, `140,450 allocs/op` |
| `BenchmarkRemoteCatalogSnapshotCodecs/BinaryDecode` | `1,832,840 ns/op`, `3,698,208 B/op`, `104,996 allocs/op` |
| `BenchmarkRemoteCatalogSnapshotCodecs/JSONEncode` | `2,840,958 ns/op`, `2,998,596 B/op`, `4 allocs/op` |
| `BenchmarkRemoteCatalogSnapshotCodecs/GobEncode` | `2,748,694 ns/op`, `7,948,608 B/op`, `75 allocs/op` |
| `BenchmarkRemoteCatalogSnapshotCodecs/BinaryEncode` | `907,512 ns/op`, `2,048,080 B/op`, `2 allocs/op` |

SQLite file-size metrics captured during the DB benchmarks:

| Benchmark | DB | WAL | SHM |
| --- | ---: | ---: | ---: |
| `BenchmarkCachedCatalogLoad` | `4,096` | `3,366,072` | `32,768` |
| `BenchmarkSQLiteSaveRemoteCatalog` unchanged | `4,403,200` | `5,034,672` | `32,768` |
| `BenchmarkSQLiteSaveRemoteCatalogChangedOne` | `4,403,200` | `5,347,792` | `32,768` |
| `BenchmarkSQLiteSaveScannerCache` | `528,384` | `4,573,232` | `32,768` |
| `BenchmarkSQLiteQueryInstallMD5s` | `126,976` | `4,136,512` | `32,768` |

The remote catalog save benchmark now separates refresh cases. `BenchmarkSQLiteSaveRemoteCatalog` is the common unchanged-refresh path and should stay low because it updates only fetched metadata after hashing. `BenchmarkSQLiteSaveRemoteCatalogInitial` is the first full custom-binary snapshot plus compatibility-row write. `BenchmarkSQLiteSaveRemoteCatalogChangedOne` represents a mostly unchanged catalog where one addon row changed and the versioned snapshot is rewritten. Existing JSON snapshots remain readable and are upgraded to the binary format on the next unchanged refresh.

## Frontend Catalog Benchmarks

| Benchmark | Result |
| --- | ---: |
| `remote search score over large cached catalog` | `900.89 hz`, `1.1100 ms mean`, `1.5898 ms p99` |
| `remote filter metadata preparation` | `304.30 hz`, `3.2862 ms mean`, `3.5004 ms p99` |
| `remote catalog indexed filter/sort` | `1,732.54 hz`, `0.5772 ms mean`, `0.5954 ms p99` |

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
