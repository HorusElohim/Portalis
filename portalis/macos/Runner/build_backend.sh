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
rm -rf "$OUTPUT_DIR/backend.framework"
mkdir -p "$OUTPUT_DIR/backend.framework/Versions/A/Resources"

# Copy compiled dylib to the expected framework binary name
if [[ "$RUST_PROFILE" == "release" ]]; then
  SRC_LIB="$RUST_DIR/target/release/libbackend.dylib"
else
  SRC_LIB="$RUST_DIR/target/debug/libbackend.dylib"
fi
cp "$SRC_LIB" "$OUTPUT_DIR/backend.framework/Versions/A/backend"

# Ensure the framework binary has a valid install name for embedding
if command -v install_name_tool >/dev/null 2>&1; then
  install_name_tool -id "@rpath/backend.framework/backend" "$OUTPUT_DIR/backend.framework/Versions/A/backend" || true
fi

# Write a minimal Info.plist so Xcode recognizes the embedded framework
cat > "$OUTPUT_DIR/backend.framework/Versions/A/Resources/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>backend</string>
  <key>CFBundleIdentifier</key>
  <string>com.portalis.backend</string>
  <key>CFBundleVersion</key>
  <string>1.0</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0</string>
  <key>CFBundlePackageType</key>
  <string>FMWK</string>
  <key>CFBundleExecutable</key>
  <string>backend</string>
  <key>MinimumOSVersion</key>
  <string>10.15</string>
  <key>CFBundleSupportedPlatforms</key>
  <array>
    <string>MacOSX</string>
  </array>
</dict>
</plist>
PLIST

# Create framework symlinks for macOS bundle compatibility
pushd "$OUTPUT_DIR/backend.framework" >/dev/null
mkdir -p Versions
pushd Versions >/dev/null
ln -sfn A Current
popd >/dev/null
mkdir -p Resources
cp Versions/Current/Resources/Info.plist Info.plist
cp Versions/Current/Resources/Info.plist Resources/Info.plist
ln -sfn Versions/Current/backend backend
popd >/dev/null

echo "✅ Rust backend copied to $OUTPUT_DIR/backend.framework"
