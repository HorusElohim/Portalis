# Build Guide – Flutter + Rust (flutter_rust_bridge)

## Overview
- This app integrates Rust into Flutter via flutter_rust_bridge (FRB) 2.11.1.
- Desktop, Web, iOS and Android work; Windows/Linux TODOs are below.
- Generated bindings live under `lib/bridge_generated` (Dart) and `rust/backend/src/api.rs` (Rust).

## Global prerequisites
- Flutter SDK 3.32.x, Xcode (macOS), Android SDK + NDK, Chrome.
- Rust toolchain (rustup) and cargo.
- Run once in `portalis/`: `flutter pub get`.
- Optional but recommended to sync codegen: `./tool/frb_build.sh <platform>`.

## macOS (Desktop)
- Just run: `flutter run -d macos`.
- The Xcode project has a build phase (`macos/Runner/build_backend.sh`) that builds and embeds the Rust dylib per Debug/Release.
- FRB’s default loader is used in Dart (`RustLib.init()`), so no manual `DynamicLibrary.open` is needed.

## Web (WASM)
- Prereqs (once):
  - `rustup target add wasm32-unknown-unknown`
  - `cargo install wasm-bindgen-cli`
- Build WASM glue: `./tool/frb_build.sh web` (produces `web/pkg/backend.js` and `web/pkg/backend_bg.wasm`).
- Run: `flutter run -d chrome`.
- Notes:
  - Dev server may print a cross-origin isolation warning; current API uses a sync binding to avoid worker threads in dev.

## iOS
- Prereqs (once):
  - `rustup target add aarch64-apple-ios`
  - `rustup target add aarch64-apple-ios-sim`
- Build/run: `flutter run -d ios`.
- What happens:
  - Xcode build phase runs `ios/Runner/build_rust_ios.sh` to build `libbackend.dylib` for device/simulator, wrap as frameworks, then package `ios/Frameworks/backend.xcframework`.
  - The XCFramework is linked and embedded (CodeSignOnCopy) into the app; FRB loads `backend.framework/backend` automatically.
- If needed, build once manually: `sh ios/Runner/build_rust_ios.sh` and verify `ios/Frameworks/backend.xcframework` exists.

## Android
- Prereqs (once):
  - Install NDK via Android Studio (SDK Manager → SDK Tools → NDK).
  - `cargo install cargo-ndk`
  - `rustup target add aarch64-linux-android x86_64-linux-android armv7-linux-androideabi`
- Build/run: `flutter run -d android`.
- What happens:
  - Gradle hooks run `android/build_rust_android.sh` before each variant to produce `libbackend.so` for ABIs and copy them to `android/app/src/main/jniLibs/**`.
  - FRB loads `libbackend.so` automatically.

## Windows – TODO
- Build `backend.dll` (`cargo build --release` for MSVC triplet) and ensure the DLL is next to the app exe at runtime.
- Optionally add a CMake/custom step to copy `backend.dll` into the bundle. Wire into the Windows CMakeLists if needed.

## Linux – TODO
- Build `libbackend.so` and copy into the app bundle’s `lib/` directory.
- Optionally extend `linux/CMakeLists.txt` to copy the `.so` from `rust/backend/target/<profile>` into the bundle.

## Regenerating FRB bindings
- One-shot codegen (and platform builds where relevant):
  - `./tool/frb_build.sh macos`
  - `./tool/frb_build.sh ios`
  - `./tool/frb_build.sh android`
  - `./tool/frb_build.sh web`
- Notes:
  - Uses `flutter_rust_bridge_codegen generate` with `--rust-root rust/backend --rust-input crate`.
  - FRB version pinned to 2.11.1; if you upgrade FRB crates, re-run codegen.

## Troubleshooting
- iOS install error: missing or invalid CFBundleExecutable in `backend.framework`.
  - Run `sh ios/Runner/build_rust_ios.sh` (the script now writes a complete Info.plist and fixes `install_name`).
- iOS runtime dlopen error for `backend.framework/backend`.
  - Ensure `ios/Frameworks/backend.xcframework` exists; rebuild; then `flutter run -d ios`.
- Web “Unexpected token ‘export’” or `wasm_bindgen is not defined`.
  - Rebuild with `./tool/frb_build.sh web` (we output `--target no-modules`).
- Web worker/DataCloneError on dev server.
  - Expected without cross-origin isolation; current API uses sync to avoid workers.
- Android: `cargo-ndk not found` or targets missing.
  - `cargo install cargo-ndk` and add Android Rust targets (see prerequisites), then rebuild.



Quick references
- Scripts:
  - `tool/frb_build.sh` – FRB codegen and platform helpers
  - `macos/Runner/build_backend.sh` – macOS dylib build + copy
  - `ios/Runner/build_rust_ios.sh` – iOS XCFramework builder (device + simulator)
  - `android/build_rust_android.sh` – Android `.so` builder (cargo-ndk)

