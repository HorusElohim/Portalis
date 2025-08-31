#!/usr/bin/env bash
set -euo pipefail

# Build Rust shared libraries for Android ABIs and copy into jniLibs.

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CRATE_DIR="$ROOT_DIR/rust/backend"
APP_DIR="$ROOT_DIR/android/app"
JNILIBS_DIR="$APP_DIR/src/main/jniLibs"

# Detect build profile; default to release for smaller libs
BUILD_PROFILE="${1:-release}"
if [[ "$BUILD_PROFILE" != "debug" && "$BUILD_PROFILE" != "release" ]]; then
  BUILD_PROFILE=release
fi

mkdir -p "$JNILIBS_DIR"

if command -v cargo-ndk >/dev/null 2>&1; then
  echo "==> Using cargo-ndk to build Android libs ($BUILD_PROFILE)"
  pushd "$CRATE_DIR" >/dev/null
  if [[ "$BUILD_PROFILE" == "release" ]]; then
    cargo ndk -o "$JNILIBS_DIR" -t arm64-v8a -t x86_64 -t armeabi-v7a build --release
  else
    cargo ndk -o "$JNILIBS_DIR" -t arm64-v8a -t x86_64 -t armeabi-v7a build
  fi
  popd >/dev/null
else
  echo "(error) cargo-ndk not found. Install with: cargo install cargo-ndk" >&2
  echo "        Then run: rustup target add aarch64-linux-android x86_64-linux-android armv7-linux-androideabi" >&2
  exit 1
fi

echo "✅ JNI libs are in: $JNILIBS_DIR"

