#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

GIT_VERSION=$(git describe --tags --always 2>/dev/null || echo "dev")
GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "none")
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

echo "building Scribe $GIT_VERSION ($GIT_COMMIT) $BUILD_DATE"

wails build -trimpath -ldflags "-s -w -X main.version=$GIT_VERSION -X main.commit=$GIT_COMMIT -X main.date=$BUILD_DATE" "$@"

if [ -f build/bin/Scribe ]; then
  SIZE=$(du -h build/bin/Scribe | cut -f1)
  echo "built: build/bin/Scribe ($SIZE)"
elif [ -f build/bin/Scribe.exe ]; then
  SIZE=$(du -h build/bin/Scribe.exe | cut -f1)
  echo "built: build/bin/Scribe.exe ($SIZE)"
fi

if command -v upx >/dev/null 2>&1; then
  echo "compressing with upx..."
  if [ -f build/bin/Scribe.exe ]; then
    upx --best --lzma build/bin/Scribe.exe
  elif [ -f build/bin/Scribe ]; then
    upx --best --lzma build/bin/Scribe
  fi
  echo "done"
fi
