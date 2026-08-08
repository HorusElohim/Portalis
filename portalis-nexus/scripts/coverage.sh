#!/usr/bin/env bash
set -euo pipefail

# Excluded as platform adapters, per SPEC.md section 18:
#   apps/server/src/main.rs      process bootstrap
#   apps/server/src/socket.rs    WebSocket plumbing driven by covered decisions
#   crates/client/src/transport  socket actor driven by covered decisions
#   demo/                        runnable examples, exercised by running them
#   generated protobuf code and integration tests
cargo llvm-cov \
  --workspace \
  --all-features \
  --ignore-filename-regex 'apps/server/src/(main|socket)\.rs|crates/client/src/transport/.*\.rs|crates/client/tests/.*\.rs|demo/src/.*\.rs|portalis\.protocol\.v1\.rs' \
  --fail-under-lines 100 \
  --fail-under-functions 100 \
  --fail-under-regions 100
