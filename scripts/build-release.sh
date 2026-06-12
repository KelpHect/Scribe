#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

GIT_VERSION=$(git describe --tags --always 2>/dev/null || echo "dev")
GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "none")
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
WINDOWS_GUI_FLAG=""
if [ "$(go env GOOS)" = "windows" ]; then
  WINDOWS_GUI_FLAG="-H windowsgui "
fi

echo "building Scribe $GIT_VERSION ($GIT_COMMIT) $BUILD_DATE"

if [ -n "${SCRIBE_PGO_PROFILE:-}" ]; then
  if [ ! -f "$SCRIBE_PGO_PROFILE" ]; then
    echo "SCRIBE_PGO_PROFILE does not exist: $SCRIBE_PGO_PROFILE" >&2
    exit 1
  fi
  export GOFLAGS="${GOFLAGS:-} -pgo=$SCRIBE_PGO_PROFILE"
  echo "using Go PGO profile: $SCRIBE_PGO_PROFILE"
fi

build_args=(
  task
  build
  "LD_FLAGS=-s -w ${WINDOWS_GUI_FLAG}-X main.version=$GIT_VERSION -X main.commit=$GIT_COMMIT -X main.date=$BUILD_DATE"
)

if [ "$(go env GOOS)" = "linux" ]; then
  build_args+=("EXTRA_TAGS=gtk3")
fi

wails3 "${build_args[@]}" "$@"

if [ -f bin/Scribe ]; then
  SIZE=$(du -h bin/Scribe | cut -f1)
  echo "built: bin/Scribe ($SIZE)"
elif [ -f bin/Scribe.exe ]; then
  SIZE=$(du -h bin/Scribe.exe | cut -f1)
  echo "built: bin/Scribe.exe ($SIZE)"
fi

if command -v upx >/dev/null 2>&1; then
  echo "compressing with upx..."
  if [ -f bin/Scribe.exe ]; then
    upx --best --lzma bin/Scribe.exe
  elif [ -f bin/Scribe ]; then
    upx --best --lzma bin/Scribe
  fi
  echo "done"
fi
