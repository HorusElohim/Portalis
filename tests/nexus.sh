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

# The demo set is the executable changelog, and it doubles as the acceptance
# suite: each binary asserts what its step delivered against production
# components, so a format or a rule that regresses fails here too. Only the
# numbered set runs — those are headless and self-checking by construction.
for source in demo/src/bin/[0-9]*.rs; do
  name="$(basename "$source" .rs)"
  echo "[demo] $name"
  cargo run --quiet -p portalis-nexus-demo --bin "$name"
done
cargo build --locked --release -p portalis-nexus-server
