#!/usr/bin/env bash
set -euo pipefail

# Excluded as platform adapters, per SPEC.md section 18:
#   apps/server/src/main.rs      process bootstrap
#   apps/server/src/socket.rs    WebSocket plumbing driven by covered decisions
#   apps/server/src/mongo/mod.rs MongoDB driver plumbing (see below)
#   crates/client/src/transport  socket actor driven by covered decisions
#   demo/                        runnable examples, exercised by running them
#   generated protobuf code and integration tests
#
# The MongoDB adapter is exercised for real by apps/server/tests/mongo.rs
# against a replica set in Docker, including registration transactions, unique
# indexes, compare-and-set, a stopped server, a standalone server, and bad
# connection strings. What remains uncovered is driver-internal error
# propagation that cannot be triggered deterministically: a transaction that
# fails to start rather than to commit, and a cursor that dies between opening
# and collecting. Its decisions live in mongo/documents.rs and store.rs, which
# stay measured at 100%.
# Each crate is compiled twice, with and without cfg(test), and both builds are
# measured. A path reached only by unit tests or only by integration tests is
# therefore uncovered in the other build, so both layers exercise the same code.
cargo llvm-cov \
  --workspace \
  --all-features \
  --ignore-filename-regex 'apps/server/src/(main|socket)\.rs|apps/server/src/mongo/mod\.rs|crates/client/src/transport/.*\.rs|crates/client/tests/.*\.rs|demo/src/.*\.rs|portalis\.protocol\.v1\.rs' \
  --fail-under-lines 100 \
  --fail-under-functions 100 \
  --fail-under-regions 100
