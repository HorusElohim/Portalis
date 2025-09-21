#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."
PORTALIS_DIR="$PROJECT_ROOT/portalis"

BACKEND_SCRIPT="$SCRIPT_DIR/backend.sh"
FRONTEND_SCRIPT="$SCRIPT_DIR/frontend.sh"

if [[ ! -x $BACKEND_SCRIPT ]]; then
  echo "[ERROR] Missing backend script at $BACKEND_SCRIPT" >&2
  exit 1
fi

if [[ ! -x $FRONTEND_SCRIPT ]]; then
  echo "[ERROR] Missing frontend script at $FRONTEND_SCRIPT" >&2
  exit 1
fi

pushd "$PROJECT_ROOT" >/dev/null

trap 'popd >/dev/null' EXIT

"$BACKEND_SCRIPT" "$@"
"$FRONTEND_SCRIPT" "$@"
