#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."

NEXUS_SCRIPT="$SCRIPT_DIR/nexus.sh"
FRONTEND_SCRIPT="$SCRIPT_DIR/frontend.sh"

if [[ ! -x $NEXUS_SCRIPT ]]; then
  echo "[ERROR] Missing Nexus script at $NEXUS_SCRIPT" >&2
  exit 1
fi

if [[ ! -x $FRONTEND_SCRIPT ]]; then
  echo "[ERROR] Missing frontend script at $FRONTEND_SCRIPT" >&2
  exit 1
fi

pushd "$PROJECT_ROOT" >/dev/null

trap 'popd >/dev/null' EXIT

"$NEXUS_SCRIPT" "$@"
"$FRONTEND_SCRIPT" "$@"
