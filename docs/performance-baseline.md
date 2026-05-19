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
| `BenchmarkScanLargeAddOnsDirectory` | `3,109,151 ns/op`, `6,050,225 B/op`, `31,487 allocs/op` |
| `BenchmarkSQLiteOpenDB` | `1,092,093 ns/op`, `270,269 B/op`, `3,908 allocs/op` |
| `BenchmarkMatchLargeCatalog` | `1,040,818 ns/op`, `1,405,279 B/op`, `23,037 allocs/op` |
| `BenchmarkCachedCatalogLoad` | `1,791,611 ns/op`, `4,752,663 B/op`, `28,111 allocs/op` |
| `BenchmarkSQLiteSaveRemoteCatalog` unchanged | `10,665,252 ns/op`, `11,311,168 B/op`, `98,100 allocs/op` |
| `BenchmarkSQLiteSaveRemoteCatalogChangedOne` | `28,324,695 ns/op`, `23,294,851 B/op`, `385,007 allocs/op` |
| `BenchmarkSQLiteSaveRemoteCatalogInitial` | `149,916,354 ns/op`, `25,806,882 B/op`, `258,962 allocs/op` |
| `BenchmarkSQLiteSaveScannerCache` | `5,520,206 ns/op`, `1,366,181 B/op`, `8,317 allocs/op` |
| `BenchmarkSQLiteQueryInstallMD5s` | `1,649,477 ns/op`, `463,931 B/op`, `12,085 allocs/op` |
| `BenchmarkRemoteSearchLargeCatalog` | `492,973 ns/op`, `300,992 B/op`, `13,908 allocs/op` |

Snapshot codec benchmark for the 7k-addon fixture:

| Benchmark | Result |
| --- | ---: |
| `BenchmarkRemoteCatalogSnapshotCodecs/JSONDecode` | `20,327,855 ns/op`, `9,569,075 B/op`, `105,025 allocs/op` |
| `BenchmarkRemoteCatalogSnapshotCodecs/GobDecode` | `4,040,195 ns/op`, `6,140,640 B/op`, `140,450 allocs/op` |
| `BenchmarkRemoteCatalogSnapshotCodecs/BinaryDecode` | `973,736 ns/op`, `2,354,144 B/op`, `28,002 allocs/op` |
| `BenchmarkRemoteCatalogSnapshotCodecs/JSONEncode` | `2,905,049 ns/op`, `3,085,404 B/op`, `4 allocs/op` |
| `BenchmarkRemoteCatalogSnapshotCodecs/GobEncode` | `2,351,200 ns/op`, `7,948,608 B/op`, `75 allocs/op` |
| `BenchmarkRemoteCatalogSnapshotCodecs/BinaryEncode` | `780,722 ns/op`, `2,048,080 B/op`, `2 allocs/op` |

SQLite file-size metrics captured during the DB benchmarks:

| Benchmark | DB | WAL | SHM |
| --- | ---: | ---: | ---: |
| `BenchmarkCachedCatalogLoad` | `4,096` | `3,366,072` | `32,768` |
| `BenchmarkSQLiteSaveRemoteCatalog` unchanged | `4,403,200` | `5,141,792` | `32,768` |
| `BenchmarkSQLiteSaveRemoteCatalogChangedOne` | `4,403,200` | `5,290,112` | `32,768` |
| `BenchmarkSQLiteSaveScannerCache` | `528,384` | `4,573,232` | `32,768` |
| `BenchmarkSQLiteQueryInstallMD5s` | `126,976` | `4,136,512` | `32,768` |

The remote catalog save benchmark now separates refresh cases. `BenchmarkSQLiteSaveRemoteCatalog` is the common unchanged-refresh path and should stay low because it updates only fetched metadata after hashing. `BenchmarkSQLiteSaveRemoteCatalogInitial` is the first full custom-binary snapshot plus compatibility-row write. `BenchmarkSQLiteSaveRemoteCatalogChangedOne` represents a mostly unchanged catalog where one addon row changed and the versioned snapshot is rewritten. Existing JSON snapshots remain readable and are upgraded to the binary format on the next unchanged refresh. Binary snapshot decoding uses immutable snapshot bytes for zero-copy strings; do not mutate decoded snapshot blobs.

## Frontend Catalog Benchmarks

| Benchmark | Result |
| --- | ---: |
| `remote search score over large cached catalog` | `863.99 hz`, `1.1574 ms mean`, `2.1965 ms p99` |
| `remote filter metadata preparation` | `331.29 hz`, `3.0185 ms mean`, `4.3226 ms p99` |
| `remote catalog indexed filter/sort` | `1,763.78 hz`, `0.5670 ms mean`, `0.6865 ms p99` |

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
