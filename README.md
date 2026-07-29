# Portalis

**Portalis** is a Flutter + Rust starter kit for building cross-platform applications that pair a Rust core with a Flutter UI using [`flutter_rust_bridge`](https://cargo.dev/rust-bridge). The template bootstraps desktop, mobile, and web targets, wraps common tooling in helper scripts, and ships with GitHub Actions build pipelines so you can focus on product code instead of plumbing.

![Portalis logo](./doc/portalis-logo.png)

## Highlights
- **Rust-powered core** – Shared business logic compiled to native libraries or WebAssembly.
- **Flutter UI** – A single Dart codebase that renders on Android, iOS, macOS, Windows, Linux, and the web.
- **First-class tooling** – One-line setup wizards (`setup/wizard_linux.sh`, `setup/wizard_darwin.sh`, `setup/wizard_windows.ps1`), consolidated test scripts, and reproducible builds.
- **Ready-to-run CI** – GitHub Actions pipeline that tests Rust and Flutter code, then builds platform artifacts.
- **Template friendly** – Automated migration assistant rewrites names, CI defaults, and docs for your next project.

## Repository Layout
```
├── doc/                  # Build & setup guides (see doc/build.md)
├── portalis/             # Flutter project (Dart UI + Rust FFI bindings)
│   ├── lib/              # Flutter widgets and FRB-generated API surface
│   ├── rust/backend/     # Rust crate compiled into native libs / wasm
│   └── tool/             # Platform-specific build helpers
├── setup/                # Environment bootstrap scripts for Linux, macOS & Windows
├── tests/                # Shell helpers to run Rust/Flutter test suites
├── scripts/              # Utility scripts (e.g., project migration)
└── .github/              # GitHub Actions workflow and composite actions
```

## Prerequisites
Install the toolchains listed below before working on the project:

- Flutter SDK 3.32.x (stable channel recommended)
- Rust toolchain via `rustup`
- Android Studio (SDK + NDK) for mobile builds
- Xcode for iOS/macOS builds on macOS hosts
- Chrome (or another Flutter-supported browser) for web builds

To accelerate setup on fresh machines, run the platform wizard that matches your OS:

- Linux: `./setup/wizard_linux.sh`
- macOS: `./setup/wizard_darwin.sh`
- Windows: `powershell -ExecutionPolicy Bypass -File .\setup\wizard_windows.ps1`

Each wizard installs common dependencies, configures environment variables, and validates with `flutter doctor` plus `rustc`/`cargo` checks. Re-running is safe and idempotent.

## Quick Start
1. Clone the repository and enter it: `git clone ... && cd Portalis`
2. Install prerequisites (or run the OS wizard above).
3. Fetch Flutter packages: `cd portalis && flutter pub get`
4. Verify Rust integration: `./tests/all.sh`
5. Launch a target, e.g.:
   - Android: `flutter run -d android`
   - Web: `flutter run -d chrome`
   - macOS: `flutter run -d macos`

Detailed build instructions for every platform live in [`doc/build.md`](doc/build.md).

## Testing
Use the scripts in `tests/` to exercise the codebase consistently:

- `./tests/backend.sh` – Runs `cargo test` for the Rust crate.
- `./tests/frontend.sh` – Runs `flutter pub get`, `flutter analyze`, and `flutter test --no-pub`.
- `./tests/all.sh` – Executes backend then frontend checks (same sequence used in CI).

CI invokes `./tests/all.sh` first and only builds artifacts if all suites pass.

## Continuous Integration
The GitHub Actions workflow (`.github/workflows/pipeline.yml`) executes the following jobs on pushes and pull requests:

1. **🧪 Tests** – Installs toolchains, runs `./tests/all.sh` (Rust + Flutter checks).
2. **Platform builds** – Each downloads the repo, reuses cached toolchains, and produces release artifacts. Parallel jobs for: 
* 🕸️ Web
* 🤖 Android
* 🐧 Linux
* 🍎 macOS
* 📱 iOS
* 🪟 Windows

3. **🧾 Summary** – Publishes artifact links and version metadata to the workflow summary.

Composite actions in `.github/actions/` encapsulate platform-specific build steps so they can be reused or adapted in other workflows.

## Tooling & Scripts
- `setup/wizard_linux.sh` / `setup/wizard_darwin.sh` / `setup/wizard_windows.ps1` – System bootstrap.
- `tests/*.sh` – Test runners used locally and in CI.
- `scripts/project_migration.py` – Migration Assistant (see below).
- `portalis/tool/frb_build.sh` – Runs `flutter_rust_bridge` code generation for specific targets.
- `portalis/tool/build_windows.ps1` – Regenerates FRB bindings and builds the Windows runner with the Rust DLL.

## Using Portalis as a GitHub Template

1. In the upstream repository, navigate to **Settings → General → Template repository** and enable it.
2. Consumers click **Use this template → Create a new repository** to spawn their project with a single initial commit (no shared history).
3. After GitHub finishes provisioning, clone the new repository locally and follow the steps below (migration script, tests, etc.).
4. If multiple starter branches are required, check **Include all branches** when creating the repo from the template.
5. Because template-derived repos have independent histories, future updates should be pulled in manually (e.g., cherry-pick or copy files).

## Template Migration Assistant
Portalis doubles as a starting point for other products. The `scripts/project_migration.py` utility rewrites template identifiers (docs, CI defaults, Dart package imports, etc.) to match your new project.

Use it after forking the repo:

```bash
# From the repository root
./scripts/project_migration.py --slug your_app --app-title "Your App"
```

- `--slug` must satisfy Flutter’s package naming rules (lowercase letters, digits, underscores).
- `--app-title` is optional; if omitted, Title Case is derived from the slug.

After the script runs:
1. Review the printed “Updated files” list and the TODO reminders for remaining platform bundle IDs.
2. Rename the `portalis/` directory to your slug if desired and adjust imports or paths it reports.
3. Run the test suite (`./tests/all.sh`) to confirm builds.
4. Commit the changes and push to your fork or downstream repository.

## Contributing
Pull requests are welcome. Please ensure all tests pass (`./tests/all.sh`) and document any platform-specific considerations inside `doc/` before submitting.

## License

This project is distributed under the Apache License 2.0. See the `LICENSE` file for the full terms.

---

Happy coding!
