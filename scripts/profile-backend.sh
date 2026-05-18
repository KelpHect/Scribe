#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OUT_DIR="${OUT_DIR:-build/reports/profiles}"
BENCHTIME="${BENCHTIME:-2s}"
mkdir -p "$OUT_DIR"
trap 'rm -f Scribe.test scanner.test esoui.test' EXIT

run_profile() {
  local name="$1"
  local pkg="$2"
  local bench="$3"
  local cpu="$OUT_DIR/${name}.cpu.pprof"
  local mem="$OUT_DIR/${name}.mem.pprof"

  echo "== ${name} =="
  go test "$pkg" -run '^$' -bench "$bench" -benchmem -benchtime "$BENCHTIME" -cpuprofile "$cpu" -memprofile "$mem"
  echo "-- CPU top (${cpu}) --"
  go tool pprof -top "$cpu" | sed -n '1,20p'
  echo "-- Memory top (${mem}) --"
  go tool pprof -top "$mem" | sed -n '1,20p'
  echo
}

run_profile "scanner-scan" "./internal/scanner" "BenchmarkScanLargeAddOnsDirectory"
run_profile "esoui-cache-load" "./internal/esoui" "BenchmarkCachedCatalogLoad"
run_profile "esoui-match-search" "./internal/esoui" "BenchmarkMatchLargeCatalog|BenchmarkRemoteSearchLargeCatalog"
run_profile "dependency-resolution" "." "BenchmarkMissingDependencyResolution"

cat <<MSG
Profiles written to ${OUT_DIR}.
Open an interactive profile with:
  go tool pprof ${OUT_DIR}/scanner-scan.cpu.pprof

Generated profile output is ignored by git.
MSG
