#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if command -v wails3 >/dev/null 2>&1; then
  WAILS=(wails3)
else
  WAILS=(go run github.com/wailsapp/wails/v3/cmd/wails3@v3.0.0-alpha.99)
fi

build_args=(build)
if [ "$(go env GOOS)" = "linux" ]; then
  build_args+=(-tags gtk3)
fi

git diff --check
"${WAILS[@]}" "${build_args[@]}"
npm --prefix frontend run check
npm --prefix frontend run lint:check
npm --prefix frontend run format:check
npm --prefix frontend run test
go test ./...
