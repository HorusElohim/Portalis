#!/usr/bin/env bash
set -euo pipefail

# This script builds the Rust backend for macOS and copies it into the app bundle's Frameworks folder.

# Resolve paths
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RUST_DIR="$PROJECT_DIR/../rust/backend"

# Xcode-provided variables for the current build
# Fallback to Debug layout if not present (e.g., manual invocation)
TARGET_BUILD_DIR_DEFAULT="$PROJECT_DIR/../build/macos/Build/Products/Debug"
WRAPPER_NAME_DEFAULT="portalis.app"

TARGET_BUILD_DIR="${TARGET_BUILD_DIR:-$TARGET_BUILD_DIR_DEFAULT}"
WRAPPER_NAME="${WRAPPER_NAME:-$WRAPPER_NAME_DEFAULT}"
CONFIGURATION="${CONFIGURATION:-Debug}"

OUTPUT_DIR="$TARGET_BUILD_DIR/$WRAPPER_NAME/Contents/Frameworks"

# Choose Rust profile based on Xcode configuration
if [[ "$CONFIGURATION" == "Release" || "$CONFIGURATION" == "Profile" ]]; then
  RUST_PROFILE=release
else
  RUST_PROFILE=debug
fi

echo "Building Rust backend ($RUST_PROFILE) into: $OUTPUT_DIR"

pushd "$RUST_DIR" >/dev/null
if [[ "$RUST_PROFILE" == "release" ]]; then
  cargo build --release
else
  cargo build
fi
popd >/dev/null

# Ensure Framework destination exists
mkdir -p "$OUTPUT_DIR/backend.framework"

# Copy compiled dylib to the expected framework binary name
if [[ "$RUST_PROFILE" == "release" ]]; then
  SRC_LIB="$RUST_DIR/target/release/libbackend.dylib"
else
  SRC_LIB="$RUST_DIR/target/debug/libbackend.dylib"
fi
cp "$SRC_LIB" "$OUTPUT_DIR/backend.framework/backend"

echo "✅ Rust backend copied to $OUTPUT_DIR/backend.framework"
