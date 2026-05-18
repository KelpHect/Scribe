#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if command -v wails >/dev/null 2>&1; then
  WAILS=(wails)
else
  WAILS=(go run github.com/wailsapp/wails/v2/cmd/wails@v2.12.0)
fi

build_args=(build)
if [ "$(go env GOOS)" = "linux" ]; then
  build_args+=(-tags webkit2_41)
fi

"${WAILS[@]}" "${build_args[@]}"
npm --prefix frontend run check
go test ./...
