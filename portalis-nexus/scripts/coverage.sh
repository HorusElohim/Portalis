#!/usr/bin/env bash
set -euo pipefail

cargo llvm-cov \
  --workspace \
  --all-features \
  --branch \
  --ignore-filename-regex 'apps/server/src/main\.rs|portalis\.protocol\.v1\.rs' \
  --fail-under-lines 100 \
  --fail-under-branches 100
