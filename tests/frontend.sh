#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

PORTALIS_DIR="$ROOT_DIR/portalis"
if [[ ! -d $PORTALIS_DIR ]]; then
  echo "[ERROR] Portalis project not found at $PORTALIS_DIR" >&2
  exit 1
fi

pushd "$PORTALIS_DIR" >/dev/null

if ! command -v flutter >/dev/null 2>&1; then
  echo "[ERROR] flutter not found on PATH" >&2
  exit 1
fi

echo "==> flutter pub get"
flutter pub get

echo "==> flutter analyze"
flutter analyze

echo "==> flutter test --no-pub ${*:+${*}}"
flutter test --no-pub "$@"

popd >/dev/null
