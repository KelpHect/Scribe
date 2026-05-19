#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

OUT_DIR="${SCRIBE_PROFILE_OUT:-build/reports/desktop-profile}"
PPROF_URL="${SCRIBE_PPROF_URL:-http://localhost:6060/debug/pprof}"
SECONDS_TO_CAPTURE="${SCRIBE_PROFILE_SECONDS:-30}"

mkdir -p "$OUT_DIR"

echo "capturing Scribe desktop profiles from ${PPROF_URL}"
echo "start Scribe separately with SCRIBE_PPROF=1, exercise the UI, then run this script"

curl -fsS "${PPROF_URL}/profile?seconds=${SECONDS_TO_CAPTURE}" -o "${OUT_DIR}/desktop.cpu.pprof"
curl -fsS "${PPROF_URL}/heap" -o "${OUT_DIR}/desktop.heap.pprof"
curl -fsS "${PPROF_URL}/goroutine?debug=0" -o "${OUT_DIR}/desktop.goroutine.pprof"
curl -fsS "${PPROF_URL}/trace?seconds=5" -o "${OUT_DIR}/desktop.trace.out"

go tool pprof -top "${OUT_DIR}/desktop.cpu.pprof" | sed -n '1,25p' > "${OUT_DIR}/desktop.cpu.top.txt"
go tool pprof -top "${OUT_DIR}/desktop.heap.pprof" | sed -n '1,25p' > "${OUT_DIR}/desktop.heap.top.txt"
go tool pprof -top "${OUT_DIR}/desktop.goroutine.pprof" | sed -n '1,25p' > "${OUT_DIR}/desktop.goroutine.top.txt"

echo "wrote profiles to ${OUT_DIR}"
echo "CPU top:"
sed -n '1,25p' "${OUT_DIR}/desktop.cpu.top.txt"
