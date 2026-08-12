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
#
# Regions are held to 99, not 100: crates/client/src/protocol.rs carries one
# region cargo-llvm-cov's summary reports as uncovered no matter what exercises
# it. ClientProtocol's generic methods (register, authenticate, link_device)
# are compiled once per concrete DeviceSigner type — one for the crate's own
# unit tests, one shared by the integration suites (crates/client/tests/common
# defines a single TestDevice for exactly this reason, rather than one per
# file). Every line and every instantiation's own coverage is complete; llvm's
# own show and JSON-segment output over the merged profile locate no
# uncovered line or branch anywhere in the file. Only the summary table's
# region tally disagrees with itself, and it does so consistently at exactly
# one region regardless of how many concrete types exercise the code, which is
# a stable count, not a symptom of a path nothing reaches.
#
# Lines are gated on the merged profile rather than the summary percentage,
# for the same reason regions are held to 99. The summary counts a line once
# per generic instantiation, and a generic service reached through more than
# one store cannot execute every line from every one of them: the production
# store never loses a compare-and-set, and a fault-injecting double never
# completes a write. Both paths are covered; no single instantiation covers
# both. LCOV reports the merged truth — a line is tested if anything reached
# it — so the gate below fails on an uncovered line rather than on a
# percentage that disagrees with itself.
# The application package's legacy modules are outside the gate, and the
# regex says which by shape rather than by list: every legacy module is a flat
# file directly in `src/` or under `src/domain/`, while everything written for
# v3 is a directory — `src/core/`, `src/store/`, `src/projection/`. So new work
# is gated from its first line, and the exclusion shrinks on its own as
# PLAN.md's steps 6 to 12 delete the files it names. It is not a judgement
# that those files deserve less; they are being replaced, and holding a
# doomed module to 100% buys nothing.
ignore='apps/server/src/(main|socket)\.rs|apps/server/src/mongo/mod\.rs|crates/client/src/transport/.*\.rs|crates/client/tests/.*\.rs|demo/src/.*\.rs|demo-m6/src/.*\.rs|backend/src/[^/]*\.rs|backend/src/domain/.*\.rs|portalis\.protocol\.v1\.rs'
lcov="$(mktemp -t nexus-coverage)"
trap 'rm -f "$lcov"' EXIT

# `--all-features` turns on the application's `local-integration` feature,
# whose one test drives two real app processes over local ports. That test is
# worth running deliberately and is a poor gate: it depends on ports, timing
# and process startup, and it measures none of the code under the policy. Skip
# it by name rather than dropping `--all-features`, so every other
# feature-gated path stays measured.
# One invocation measures and gates. A second `cargo llvm-cov report` reading
# the same profile prints an empty table, so the numbers come from the run
# that gated them or not at all — a coverage table that disagrees with the
# gate is worse than no table.
#
# `clean` first, so a run answers only for the tests it just executed. Without
# it a file can keep the coverage of a test that has since been removed.
cargo llvm-cov clean --workspace
cargo llvm-cov \
  --workspace \
  --all-features \
  --ignore-filename-regex "$ignore" \
  --summary-only \
  --fail-under-functions 100 \
  --fail-under-regions 99 \
  -- --skip two_instances_sync

# The per-line truth the percentages above cannot show, from the same profile.
cargo llvm-cov report --ignore-filename-regex "$ignore" --lcov --output-path "$lcov"

if grep -q '^DA:[0-9]*,0$' "$lcov"; then
  echo >&2
  echo "Uncovered lines:" >&2
  awk -F'[:,]' '
    /^SF:/ { file = $2 }
    /^DA:/ && $3 == 0 { print "  " file ":" $2 }
  ' "$lcov" >&2
  exit 1
fi
