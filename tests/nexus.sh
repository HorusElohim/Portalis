#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$ROOT_DIR/portalis/rust/backend"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[ERROR] cargo not found on PATH" >&2
  exit 1
fi

if ! command -v buf >/dev/null 2>&1; then
  echo "[ERROR] buf not found on PATH" >&2
  exit 1
fi

pushd "$BACKEND_DIR" >/dev/null
trap 'popd >/dev/null' EXIT

buf lint
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features "$@"
./scripts/coverage.sh
cargo build --locked --release -p portalis-nexus-server
