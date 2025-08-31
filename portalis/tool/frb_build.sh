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
  flutter_rust_bridge_codegen generate \
    --rust-root "$CRATE" \
    --rust-input crate \
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
  if ! command -v wasm-bindgen >/dev/null 2>&1; then
    if [[ "${FRB_BOOTSTRAP:-0}" == "1" ]]; then
      echo "Installing wasm-bindgen CLI ..."
      cargo install wasm-bindgen-cli
    else
      echo "(error) wasm-bindgen CLI not found."
      echo "        Install with: cargo install wasm-bindgen-cli"
      return 1
    fi
  fi
  echo "Compiling Rust to wasm..."
  (cd "$CRATE" && cargo build --release --target wasm32-unknown-unknown)
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
  ios) maybe_codegen; echo "iOS: Codegen done. Xcode will build XCFramework via build phase." ;;
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
