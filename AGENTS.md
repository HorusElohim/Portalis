# Agent instructions

These rules apply to changes made in this repository.

## Change discipline

- Keep commits atomic: one commit should represent one coherent logical
  change. Do not mix refactors, formatting churn, generated artifacts, or
  unrelated fixes into a feature commit.
- Prefer the simplest correct design. Avoid speculative abstractions,
  duplicated sources of truth, and new dependencies when existing project
  facilities are sufficient.
- Always improve the design when touching code: remove duplication, clarify
  boundaries, and leave the next change simpler without expanding scope.
- Preserve clean object-oriented boundaries in Flutter: widgets own UI state,
  services own application workflows, and the Rust backend owns domain and
  persistence logic. In Rust, use idiomatic ownership and small focused
  modules rather than forcing classes or inheritance where they do not fit.
- Keep public APIs explicit and stable. Treat changes to Rust bridge DTOs and
  generated bindings as one atomic change.

## Keep the codebase small

- Reuse existing helpers, widgets, services, and dependencies before adding
  another abstraction or utility.
- Prefer data flow over duplicated state: keep one source of truth and derive
  display values, caches, and summaries from it.
- Keep functions, widgets, and modules focused; expose the narrowest API that
  solves the problem and delete dead code instead of preserving unused paths.
- Favor straightforward control flow and readable names over cleverness,
  defensive layers, premature extensibility, or configuration that has no
  current consumer.
- When a change makes an older workaround unnecessary, remove the workaround
  in the same commit.

## Git

- Prefix commit messages with one fitting emoji and a conventional type, for example `✨ feat: add download folder setting` or `🐛 fix: reject stale backend`.

## Versions and changelog

- Every user-visible, runtime, or bridge change must update `CHANGELOG.md`.
- Increment both versions for a release: the Flutter version in
  `portalis/pubspec.yaml` and the Rust backend version in
  `portalis/rust/backend/Cargo.toml` and its local `Cargo.lock` package entry.
- Keep the frontend and backend compatibility expectations in sync when a
  bridge schema changes.
- Generated Flutter-Rust bridge files must be regenerated whenever bridged
  Rust types or functions change. Review generated diffs; do not leave a Dart
  binding and native library built from different schemas.

## Tests and verification

- Add or update focused tests alongside behavior changes, including persistence
  and bridge DTO round-trip tests where applicable.
- Before committing, run the smallest relevant test, analysis, and build
  commands, then the broader checks when the change affects shared APIs.
- Ask the user before running build, code-generation, formatter, or test
  commands when they have requested interactive control of those commands.
- Do not claim verification that was not actually run. Record skipped checks in
  the handoff.
