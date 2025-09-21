# Build Guide – Flutter + Rust (flutter_rust_bridge)

## Overview
- This app integrates Rust into Flutter via flutter_rust_bridge (FRB) 2.11.1.
- Desktop, Web, iOS and Android work; see platform sections below for Windows and Linux notes.
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

## Windows (Desktop)

Prerequisites
- Run `setup/wizard_windows.ps1` in an elevated PowerShell to install Flutter, VS Build Tools (C++), Rust, Android SDK, etc.
- Ensure `flutter doctor` is green. The wizard prints the expected final output for reference.

Dev run (recommended)
- From repo root `portalis/` in PowerShell:
  - `./tool/build_windows.ps1`  # regenerates FRB (if available), builds Rust DLL, runs Flutter on Windows

Manual steps (equivalent)
- Build Rust DLL (release, required by FRB loader at runtime):
  - `cargo build --release --manifest-path rust/backend/Cargo.toml`
- Run Flutter on Windows:
  - `flutter pub get`
  - `flutter run -d windows`

Release build (packaged app)
- Build Rust DLL:
  - `cargo build --release --manifest-path rust/backend/Cargo.toml`
- Build Flutter bundle:
  - `flutter build windows --release`
- Copy the Rust DLL next to the runner exe so the loader can find it:
  - Copy `rust/backend/target/release/backend.dll` to `build/windows/x64/runner/Release/`
  - The final folder contains `portalis.exe`, Flutter DLLs, assets, and `backend.dll`.

Notes
- FRB’s generated config expects the DLL at `rust/backend/target/release/backend.dll` during `flutter run`; keep using `--release` for the Rust build.
- To update bindings after Rust API changes, install the codegen: `cargo install flutter_rust_bridge_codegen`, then re-run `./tool/build_windows.ps1`.

## Linux (Desktop)

Prerequisites
- Run `setup/wizard_linux.sh` (shell) to install Flutter, Android SDK bits, Rust, clang, ninja, GTK dev libs, etc.
- Ensure `flutter doctor` is green, especially the Linux desktop toolchain checks.

Dev run (recommended)
- From repo root `portalis/` in a shell:
  - `cargo build --release --manifest-path rust/backend/Cargo.toml`  # produces `libbackend.so` in `rust/backend/target/release/`
  - `flutter pub get`
  - `flutter run -d linux`

Release build (packaged app)
- Build Rust shared library:
  - `cargo build --release --manifest-path rust/backend/Cargo.toml`
- Build Flutter bundle:
  - `flutter build linux --release`
- Copy the Rust library into the bundle so the loader can find it:
  - Copy `rust/backend/target/release/libbackend.so` to `build/linux/x64/release/bundle/lib/`

Notes
- The flutter_rust_bridge loader looks in `rust/backend/target/release/` during `flutter run`; keep the Rust build in `--release` mode for parity with other platforms.
- If you change Rust APIs, rebuild bindings via `cargo install flutter_rust_bridge_codegen` (once) and rerun `./tool/frb_build.sh <platform>`.

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

