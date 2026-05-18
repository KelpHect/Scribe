#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "== Go fixture benchmarks =="
go test ./internal/scanner ./internal/esoui -run '^$' -bench 'Benchmark' -benchmem

echo
echo "== Frontend catalog benchmarks =="
npm --prefix frontend run bench -- --run

cat <<'MSG'

== Startup and memory snapshots ==
Capture cold and warm startup from Settings -> Diagnostics -> Copy export.
Cold startup: quit Scribe, remove or move the local ESOUI cache DB if you need a no-cache run, launch, wait for the first idle diagnostics capture, then copy export.
Warm startup: launch again with the existing cache and copy export after the first idle capture.

Do not enforce thresholds from these fixture benchmarks yet. Record the numbers before tightening budgets.
MSG
