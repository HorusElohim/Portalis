#!/usr/bin/env bash
# Portalis macOS Dev Environment Wizard
set -euo pipefail

INFO_COLOR="\033[36m"
OK_COLOR="\033[32m"
WARN_COLOR="\033[33m"
ERR_COLOR="\033[31m"
RESET_COLOR="\033[0m"

info() { printf "%b[INFO ] %s%b\n" "$INFO_COLOR" "$*" "$RESET_COLOR"; }
ok() { printf "%b[ OK  ] %s%b\n" "$OK_COLOR" "$*" "$RESET_COLOR"; }
warn() { printf "%b[WARN ] %s%b\n" "$WARN_COLOR" "$*" "$RESET_COLOR"; }
err() { printf "%b[ERROR] %s%b\n" "$ERR_COLOR" "$*" "$RESET_COLOR" 1>&2; }

confirm_yes() {
    local prompt="$1"
    local default="${2:-y}"
    local suffix
    if [[ "$default" =~ ^[Yy]$ ]]; then
        suffix="[Y/n]"
    else
        suffix="[y/N]"
    fi
    read -r -p "$prompt $suffix " reply || reply=""
    if [[ -z "$reply" ]]; then
        reply="$default"
    fi
    [[ "$reply" =~ ^([Yy]|yes)$ ]]
}

command_exists() { command -v "$1" >/dev/null 2>&1; }

PROFILE_FILE="$HOME/.zshrc"
mkdir -p "$(dirname "$PROFILE_FILE")"
touch "$PROFILE_FILE"

ensure_profile_line() {
    local line="$1"
    grep -Fqx "$line" "$PROFILE_FILE" >/dev/null 2>&1 || printf '%s\n' "$line" >>"$PROFILE_FILE"
}

ARCH="$(uname -m)"
info "Detected architecture: $ARCH"

if ! command_exists brew; then
    info "Installing Homebrew"
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    if [[ -f "/opt/homebrew/bin/brew" ]]; then
        export PATH="/opt/homebrew/bin:$PATH"
        ensure_profile_line 'eval "$([ -f "/opt/homebrew/bin/brew" ] && /opt/homebrew/bin/brew shellenv)"'
    elif [[ -f "/usr/local/bin/brew" ]]; then
        export PATH="/usr/local/bin:$PATH"
        ensure_profile_line 'eval "$([ -f "/usr/local/bin/brew" ] && /usr/local/bin/brew shellenv)"'
    fi
else
    ok "Homebrew already installed"
fi

if ! command_exists brew; then
    err "Homebrew is still unavailable after installation. Please install it manually and re-run." && exit 1
fi

if xcode-select -p >/dev/null 2>&1; then
    ok "Xcode Command Line Tools already available"
else
    info "Installing Xcode Command Line Tools"
    xcode-select --install || true
    warn "Please complete the Xcode Command Line Tools prompt, then re-run the wizard."
fi

info "Updating Homebrew"
brew update

info "Installing core packages"
brew install git curl unzip zip xz pkg-config cmake ninja

if ! command_exists buf; then
    info "Installing Buf for Portalis Nexus protobuf checks"
    brew install bufbuild/buf/buf || warn "Buf install failed"
else
    ok "Buf already installed"
fi

if confirm_yes "Install Visual Studio Code via Homebrew Cask?" y; then
    if ! command_exists code; then
        info "Installing VS Code"
        brew install --cask visual-studio-code || warn "VS Code install failed"
    else
        ok "VS Code already available"
    fi
fi

if confirm_yes "Install Android Studio via Homebrew Cask?" y; then
    if ! command_exists studio; then
        info "Installing Android Studio"
        brew install --cask android-studio || warn "Android Studio install failed"
    else
        ok "Android Studio already available"
    fi
fi

if ! brew list --versions openjdk@17 >/dev/null 2>&1; then
    info "Installing OpenJDK 17"
    brew install openjdk@17 || warn "OpenJDK 17 install failed"
else
    ok "OpenJDK 17 already installed"
fi

if command_exists brew && brew --prefix openjdk@17 >/dev/null 2>&1; then
    JAVA_HOME="$(brew --prefix openjdk@17)"
    export JAVA_HOME
    ok "Using JAVA_HOME: $JAVA_HOME"
    ensure_profile_line "export JAVA_HOME=\"$JAVA_HOME\""
    ensure_profile_line 'export PATH="$JAVA_HOME/bin:$PATH"'
else
    warn "OpenJDK 17 not detected. Install it manually if Android builds fail."
fi

ANDROID_HOME="$HOME/Library/Android/sdk"
mkdir -p "$ANDROID_HOME"
ensure_profile_line "export ANDROID_HOME=\"$ANDROID_HOME\""
ensure_profile_line "export ANDROID_SDK_ROOT=\"$ANDROID_HOME\""

CMDLINE_DIR="$ANDROID_HOME/cmdline-tools/latest"
ANDROID_TOOLS_URL="https://dl.google.com/android/repository/commandlinetools-mac-11076708_latest.zip"
if [[ ! -x "$CMDLINE_DIR/bin/sdkmanager" ]]; then
    info "Installing Android command-line tools"
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT
    curl -fsSL "$ANDROID_TOOLS_URL" -o "$tmpdir/cmdline-tools.zip"
    mkdir -p "$ANDROID_HOME/cmdline-tools"
    unzip -q "$tmpdir/cmdline-tools.zip" -d "$tmpdir"
    rm -rf "$CMDLINE_DIR"
    mv "$tmpdir/cmdline-tools" "$CMDLINE_DIR"
    rm -rf "$tmpdir"
    trap - EXIT
    ok "Android command-line tools ready"
fi

ensure_profile_line 'export PATH="$ANDROID_HOME/platform-tools:$PATH"'
ensure_profile_line 'export PATH="$ANDROID_HOME/emulator:$PATH"'
ensure_profile_line 'export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"'

SDKMANAGER="$CMDLINE_DIR/bin/sdkmanager"
if [[ ! -x "$SDKMANAGER" ]]; then
    err "sdkmanager not found. Aborting." && exit 1
fi

if [[ -n "${JAVA_HOME:-}" ]]; then
    yes | "$SDKMANAGER" --sdk_root="$ANDROID_HOME" --licenses >/dev/null || warn "Accepting licenses encountered issues"
else
    warn "JAVA_HOME missing; skipping automatic license acceptance"
fi

SDK_PACKAGES=(
    "platform-tools"
    "platforms;android-35"
    "build-tools;35.0.0"
    "cmdline-tools;latest"
    "emulator"
)
info "Installing Android SDK components"
yes | "$SDKMANAGER" --sdk_root="$ANDROID_HOME" "${SDK_PACKAGES[@]}" >/dev/null || warn "sdkmanager component install returned warnings"

FLUTTER_ROOT="$HOME/Development/flutter"
FLUTTER_BIN="$FLUTTER_ROOT/bin"
if [[ ! -d "$FLUTTER_ROOT" ]]; then
    info "Cloning Flutter (stable channel)"
    git clone --depth 1 --branch stable https://github.com/flutter/flutter.git "$FLUTTER_ROOT"
else
    info "Updating Flutter"
    git -C "$FLUTTER_ROOT" fetch --depth 1 origin stable
    git -C "$FLUTTER_ROOT" reset --hard origin/stable
fi

export PATH="$FLUTTER_BIN:$PATH"
ensure_profile_line "export PATH=\"$FLUTTER_BIN:\$PATH\""

"$FLUTTER_BIN/flutter" --version >/dev/null || warn "flutter --version encountered issues"

if ! command_exists rustup; then
    info "Installing Rust via rustup"
    curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable
    if [[ -f "$HOME/.cargo/env" ]]; then
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
    fi
else
    ok "Rustup already installed"
fi

if command_exists rustup; then
    info "Updating Rust toolchains"
    rustup self update
    rustup toolchain install stable
    rustup default stable
    rustup component add clippy rustfmt llvm-tools-preview || warn "Rust component setup encountered issues"
    ok "Rust stable ready"
else
    warn "rustup not available; skipping Rust setup"
fi

if command_exists cargo; then
    if ! command_exists cargo-llvm-cov; then
        info "Installing cargo-llvm-cov for Portalis Nexus coverage"
        cargo install cargo-llvm-cov --locked || warn "cargo-llvm-cov install failed"
    else
        ok "cargo-llvm-cov already installed"
    fi

    if ! command_exists flutter_rust_bridge_codegen; then
        info "Installing flutter_rust_bridge_codegen"
        cargo install flutter_rust_bridge_codegen || warn "flutter_rust_bridge_codegen install failed"
    else
        ok "flutter_rust_bridge_codegen already installed"
    fi

    if confirm_yes "Install gitmoji-rs (emoji commit assistant)?" y; then
        if ! command_exists gitmoji; then
            info "Installing gitmoji-rs"
            cargo install gitmoji-rs || warn "gitmoji-rs install failed"
        else
            ok "gitmoji-rs already installed"
        fi
    fi
else
    warn "cargo not available; skipping Rust crate installs"
fi

if command_exists code; then
    if ! code --version >/dev/null 2>&1; then
        warn "VS Code CLI not ready; skipping extension installation"
    else
        vscode_extension_installed() {
            local slug="$1"
            local dir
            local -a search_dirs=(
                "$HOME/.vscode/extensions"
                "$HOME/.vscode-oss/extensions"
                "$HOME/Library/Application Support/Code/User/extensions"
            )
            for dir in "${search_dirs[@]}"; do
                if [[ -d "$dir" ]]; then
                    shopt -s nullglob
                    local -a matches=("$dir/${slug}"*)
                    shopt -u nullglob
                    if (( ${#matches[@]} > 0 )); then
                        return 0
                    fi
                fi
            done
            return 1
        }
        install_extension() {
            local ext="$1"
            if vscode_extension_installed "$ext"; then
                ok "VS Code extension already installed: $ext"
                return
            fi
            info "Installing VS Code extension: $ext"
            if code --install-extension "$ext" --force >/dev/null 2>&1; then
                ok "Installed VS Code extension: $ext"
            else
                warn "Failed to install $ext. Launch VS Code once, then re-run the wizard."
            fi
        }
        install_extension "Dart-Code.dart-code"
        install_extension "Dart-Code.flutter"
        install_extension "rust-lang.rust-analyzer"
        install_extension "tamasfe.even-better-toml"
        install_extension "EditorConfig.EditorConfig"
    fi
else
    warn "VS Code CLI not found; skipping extension installation"
fi

if confirm_yes "Create or upgrade a local Flutter app named 'Portalis' in the current directory?" n; then
    if command_exists flutter; then
        proj_slug="portalis"
        proj_dir="$(pwd)/$proj_slug"
        alt_proj="$(pwd)/Portalis"
        if [[ -d "$proj_dir" ]]; then
            info "Project exists. Running flutter pub get"
            (cd "$proj_dir" && flutter pub get)
            target_dir="$proj_dir"
        elif [[ -d "$alt_proj" ]]; then
            info "Existing directory 'Portalis' found. Running flutter pub get"
            (cd "$alt_proj" && flutter pub get)
            target_dir="$alt_proj"
        else
            info "Creating Flutter app 'portalis'"
            flutter create "$proj_slug"
            target_dir="$proj_dir"
        fi
        ok "Portalis Flutter app ready: $target_dir"
    else
        warn "Flutter CLI not found in PATH for this session"
    fi
fi

info "Diagnostics"
if command_exists flutter; then
    flutter --version || warn "flutter --version reported issues"
    if ! flutter doctor; then
        warn "flutter doctor reported issues. Open Android Studio and accept SDK licenses, then re-run."
    fi
else
    warn "Flutter CLI missing. Open a new shell to refresh PATH."
fi

if command_exists rustc; then
    rustc --version || warn "rustc --version reported issues"
else
    warn "rustc not found in PATH"
fi

if command_exists cargo; then
    cargo --version || warn "cargo --version reported issues"
else
    warn "cargo not found in PATH"
fi

if command_exists buf; then
    buf --version || warn "buf --version reported issues"
else
    warn "buf not found in PATH"
fi

if command_exists cargo-llvm-cov; then
    cargo llvm-cov --version || warn "cargo llvm-cov --version reported issues"
else
    warn "cargo-llvm-cov not found in PATH"
fi

ok "Setup wizard finished. Open a new shell to ensure PATH updates take effect."
info "Next steps:"
printf '  1) Launch Android Studio once and finish its setup wizard.\n'
printf '  2) Run: flutter doctor --android-licenses\n'
printf '  3) Verify everything with: flutter doctor\n'
