#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

PORTALIS_DIR="$ROOT_DIR/portalis"
if [[ ! -d $PORTALIS_DIR ]]; then
  echo "[ERROR] Portalis project not found at $PORTALIS_DIR" >&2
  exit 1
fi

pushd "$PORTALIS_DIR" >/dev/null

if ! command -v cargo >/dev/null 2>&1; then
  echo "[ERROR] cargo not found on PATH" >&2
  exit 1
fi

cargo test --manifest-path rust/backend/Cargo.toml "$@"

popd >/dev/null
