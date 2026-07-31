#!/usr/bin/env bash
set -euo pipefail

# Unified helper to (optionally) regenerate FRB bindings and build native libs.
# This repo uses flutter_rust_bridge 2.x with generated files already present.
# If you have the codegen installed, we’ll run it; otherwise we’ll just build.

cd "$(dirname "$0")/.."

CRATE="rust/backend"
PLATFORM="${1:-macos}"

function maybe_codegen() {
  if ! command -v flutter_rust_bridge_codegen >/dev/null 2>&1; then
    echo "(error) flutter_rust_bridge_codegen not found."
    echo "        Install with: cargo install flutter_rust_bridge_codegen"
    exit 1
  fi
  echo "==> Regenerating flutter_rust_bridge bindings"
  # Use new CLI (2.x): rust_input expects crate paths, and rust_root points to the crate dir.
  # IMPORTANT: list every bridged module explicitly (never the bare "crate"
  # wildcard) — flutter_rust_bridge_codegen's crate-wide scan walks every
  # `mod` declaration regardless of visibility (pub/pub(crate)/private all
  # look the same to it), so a bare "crate" sweeps up internal-only modules
  # like `domain` too and fails to compile (private fields it assumes are
  # bridgeable). See rust/backend/README.md's "Flutter boundary API".
  flutter_rust_bridge_codegen generate \
    --rust-root "$CRATE" \
    --rust-input crate::bridge,crate::torrent,crate::device,crate::collections,crate::settings \
    --dart-output "lib/bridge_generated" \
    --rust-output "$CRATE/src/api.rs"
}

function build_macos() {
  echo "==> cargo build (macOS)"
  (cd "$CRATE" && cargo build --release)
  echo "Built: $CRATE/target/release/libbackend.dylib"
}

function build_linux() {
  echo "==> cargo build (Linux)"
  (cd "$CRATE" && cargo build --release)
  echo "Built: $CRATE/target/release/libbackend.so"
}

function build_windows() {
  echo "==> cargo build (Windows)"
  echo "Note: Cross-compiling Windows from non-Windows hosts is not configured here."
  (cd "$CRATE" && cargo build --release)
  echo "Built: $CRATE/target/release/backend.dll (on Windows hosts)"
}

function build_web() {
  echo "==> Web build"
  # Requirements: rustup target add wasm32-unknown-unknown, wasm-bindgen-cli installed
  # If FRB_BOOTSTRAP=1 is set, attempt to auto-install missing prerequisites.
  if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
    if [[ "${FRB_BOOTSTRAP:-0}" == "1" ]]; then
      echo "Installing Rust target wasm32-unknown-unknown ..."
      rustup target add wasm32-unknown-unknown
    else
      echo "(error) Rust target wasm32-unknown-unknown not installed."
      echo "        Install with: rustup target add wasm32-unknown-unknown"
      return 1
    fi
  fi
  echo "Compiling Rust to wasm..."
  (cd "$CRATE" && cargo build --release --target wasm32-unknown-unknown)

  # The wasm-bindgen crate version pulled in by the build (recorded in
  # Cargo.lock) must exactly match the wasm-bindgen-cli binary used to
  # generate the JS/WASM glue below. Cargo.lock for this crate isn't
  # committed (see .gitignore), so the resolved version can drift between
  # machines/CI runs; always align the CLI to whatever was actually built.
  WBG_VERSION=$(grep -A1 '^name = "wasm-bindgen"$' "$CRATE/Cargo.lock" | grep '^version' | head -1 | sed -E 's/version = "(.*)"/\1/')
  if [[ -z "$WBG_VERSION" ]]; then
    echo "(error) Could not determine wasm-bindgen version from $CRATE/Cargo.lock"
    return 1
  fi
  INSTALLED_WBG_VERSION=""
  if command -v wasm-bindgen >/dev/null 2>&1; then
    INSTALLED_WBG_VERSION=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')
  fi
  if [[ "$INSTALLED_WBG_VERSION" != "$WBG_VERSION" ]]; then
    if [[ "${FRB_BOOTSTRAP:-0}" == "1" ]]; then
      echo "Installing wasm-bindgen-cli $WBG_VERSION (found: ${INSTALLED_WBG_VERSION:-none}) ..."
      cargo install wasm-bindgen-cli --version "$WBG_VERSION" --force
    else
      echo "(error) wasm-bindgen-cli version mismatch: need $WBG_VERSION, found ${INSTALLED_WBG_VERSION:-none}."
      echo "        Install with: cargo install wasm-bindgen-cli --version $WBG_VERSION --force"
      return 1
    fi
  fi
  mkdir -p web/pkg
  echo "Generating JS/WASM glue into web/pkg ..."
  wasm-bindgen \
    --target no-modules \
    --out-dir web/pkg \
    "$CRATE/target/wasm32-unknown-unknown/release/backend.wasm"
  echo "Artifacts: web/pkg/backend.js, web/pkg/backend_bg.wasm"
}

function build_android() {
  echo "==> Android build via cargo-ndk"
  bash android/build_rust_android.sh release
}

case "$PLATFORM" in
  macos) maybe_codegen; build_macos ;;
  ios) maybe_codegen; echo "==> Building iOS XCFramework"; bash ios/Runner/build_rust_ios.sh ;;
  android) maybe_codegen; build_android ;;
  linux) maybe_codegen; build_linux ;;
  windows) maybe_codegen; build_windows ;;
  web) maybe_codegen; build_web ;;
  all)
    maybe_codegen
    build_macos || true
    build_linux || true
    build_web || true
    ;;
  *) echo "Unknown platform: $PLATFORM" >&2; exit 1 ;;
esac

echo "✅ Done: $PLATFORM"
