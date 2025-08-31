#!/usr/bin/env bash
set -euo pipefail

# Build Rust dynamic frameworks for iOS (device + simulator) and package as an XCFramework.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
IOS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJ_DIR="$IOS_DIR"
RUST_DIR="$IOS_DIR/../rust/backend"
OUT_DIR="$IOS_DIR/Frameworks"

mkdir -p "$OUT_DIR"

# Ensure required Rust targets (install if FRB_BOOTSTRAP=1)
function ensure_target() {
  local tgt="$1"
  if ! rustup target list --installed | grep -q "^${tgt}$"; then
    if [[ "${FRB_BOOTSTRAP:-0}" == "1" ]]; then
      echo "Installing Rust target ${tgt} ..."
      rustup target add "$tgt"
    else
      echo "(error) Missing Rust target: ${tgt}. Install with: rustup target add ${tgt}" >&2
      exit 1
    fi
  fi
}

ensure_target aarch64-apple-ios
ensure_target aarch64-apple-ios-sim || true

pushd "$RUST_DIR" >/dev/null
cargo build --release --target aarch64-apple-ios
if rustup target list --installed | grep -q '^aarch64-apple-ios-sim$'; then
  cargo build --release --target aarch64-apple-ios-sim
fi
popd >/dev/null

DEVICE_DYLIB="$RUST_DIR/target/aarch64-apple-ios/release/libbackend.dylib"
SIM_DYLIB_ARM64="$RUST_DIR/target/aarch64-apple-ios-sim/release/libbackend.dylib"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

create_framework() {
  local src_dylib="$1"; shift
  local dst_dir="$1"; shift
  local platform="$1"; shift  # iPhoneOS or iPhoneSimulator
  mkdir -p "$dst_dir"
  # Copy dylib and rename to framework binary name
  cp "$src_dylib" "$dst_dir/backend"
  # Ensure install_name uses @rpath so it is loadable from the embedded Frameworks directory
  if command -v install_name_tool >/dev/null 2>&1; then
    install_name_tool -id "@rpath/backend.framework/backend" "$dst_dir/backend" || true
  fi
  # Minimal Info.plist required by frameworks
  cat > "$dst_dir/Info.plist" <<'PLIST'
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
  <string>12.0</string>
</dict>
</plist>
PLIST
  # Add platform hint
  /usr/libexec/PlistBuddy -c "Add :CFBundleSupportedPlatforms array" "$dst_dir/Info.plist" 2>/dev/null || true
  /usr/libexec/PlistBuddy -c "Add :CFBundleSupportedPlatforms:0 string $platform" "$dst_dir/Info.plist" 2>/dev/null || \
  /usr/libexec/PlistBuddy -c "Set :CFBundleSupportedPlatforms:0 $platform" "$dst_dir/Info.plist" 2>/dev/null || true
}

FWK_DEV="$TMP_DIR/Device/backend.framework"
FWK_SIM="$TMP_DIR/Simulator/backend.framework"
rm -rf "$FWK_DEV" "$FWK_SIM"

create_framework "$DEVICE_DYLIB" "$FWK_DEV" "iPhoneOS"
if [[ -f "$SIM_DYLIB_ARM64" ]]; then
  create_framework "$SIM_DYLIB_ARM64" "$FWK_SIM" "iPhoneSimulator"
fi

XC_OUT="$OUT_DIR/backend.xcframework"
rm -rf "$XC_OUT"

CMD=(xcodebuild -create-xcframework -framework "$FWK_DEV")
if [[ -d "$FWK_SIM" ]]; then
  CMD+=( -framework "$FWK_SIM" )
fi
CMD+=( -output "$XC_OUT" )
"${CMD[@]}"

echo "✅ Built XCFramework (framework-based) at: $XC_OUT"
