#!/usr/bin/env bash
set -euo pipefail

cargo llvm-cov \
  --workspace \
  --all-features \
  --ignore-filename-regex 'apps/server/src/(main|socket)\.rs|crates/client/(src/transport|tests/.*)\.rs|portalis\.protocol\.v1\.rs' \
  --fail-under-lines 100 \
  --fail-under-functions 100 \
  --fail-under-regions 100
