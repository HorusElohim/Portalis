#!/usr/bin/env bash
# Portalis Linux Dev Environment Wizard
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

PROFILE_FILE="$HOME/.profile"
touch "$PROFILE_FILE"

ensure_profile_line() {
    local line="$1"
    grep -Fx "$line" "$PROFILE_FILE" >/dev/null 2>&1 || printf '%s\n' "$line" >>"$PROFILE_FILE"
}

if [[ $EUID -ne 0 ]]; then
    if command_exists sudo; then
        SUDO="sudo"
    else
        err "This wizard needs administrative privileges. Install sudo or re-run as root." && exit 1
    fi
else
    SUDO=""
fi

PKG_MANAGER=""
if command_exists apt-get; then
    PKG_MANAGER="apt"
elif command_exists dnf; then
    PKG_MANAGER="dnf"
elif command_exists pacman; then
    PKG_MANAGER="pacman"
else
    warn "No supported package manager (apt, dnf, pacman) detected. System packages must be installed manually."
fi

APT_UPDATED=0
PACMAN_SYNCED=0
install_packages() {
    if [[ -z "$PKG_MANAGER" ]]; then
        warn "Skipping installation of: $*"
        return
    fi
    case "$PKG_MANAGER" in
        apt)
            if [[ $APT_UPDATED -eq 0 ]]; then
                info "Updating apt package index"
                DEBIAN_FRONTEND=noninteractive $SUDO apt-get update -y
                APT_UPDATED=1
            fi
            DEBIAN_FRONTEND=noninteractive $SUDO apt-get install -y "$@"
            ;;
        dnf)
            $SUDO dnf install -y "$@"
            ;;
        pacman)
            if [[ $PACMAN_SYNCED -eq 0 ]]; then
                $SUDO pacman -Sy --noconfirm
                PACMAN_SYNCED=1
            fi
            $SUDO pacman -S --noconfirm --needed "$@"
            ;;
    esac
}

ARCH="$(uname -m)"
info "Detected architecture: $ARCH"

case "$PKG_MANAGER" in
    apt)
        install_packages git curl unzip zip xz-utils file build-essential libglu1-mesa clang ninja-build libgtk-3-dev pkg-config cmake mesa-utils
        ;;
    dnf)
        install_packages git curl unzip zip xz mesa-libGLU file gcc-c++ make clang ninja-build gtk3-devel pkgconfig cmake mesa-demos
        ;;
    pacman)
        install_packages git curl unzip zip xz glu base-devel clang ninja gtk3 pkgconf cmake mesa-demos
        ;;
    *)
        warn "Install git, curl, unzip, zip, xz, build tools, clang, ninja, GTK dev libs, and mesa-utils manually."
        ;;
esac

case "$PKG_MANAGER" in
    apt)
        install_packages openjdk-17-jdk
        ;;
    dnf)
        install_packages java-17-openjdk java-17-openjdk-devel
        ;;
    pacman)
        install_packages jdk17-openjdk
        ;;
    *)
        warn "Install OpenJDK 17 manually."
        ;;
esac

if command_exists javac; then
    JAVA_HOME="$(dirname "$(dirname "$(readlink -f "$(command -v javac)")")")"
    export JAVA_HOME
    ok "Detected JAVA_HOME: $JAVA_HOME"
    ensure_profile_line "export JAVA_HOME=\"$JAVA_HOME\""
else
    warn "javac not found; JAVA_HOME not configured."
fi

ANDROID_HOME="$HOME/Android/Sdk"
mkdir -p "$ANDROID_HOME"
ensure_profile_line "export ANDROID_HOME=\"$ANDROID_HOME\""
ensure_profile_line "export ANDROID_SDK_ROOT=\"$ANDROID_HOME\""

CMDLINE_DIR="$ANDROID_HOME/cmdline-tools/latest"
ANDROID_TOOLS_URL="https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip"
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

ensure_profile_line "export PATH=\"$CMDLINE_DIR/bin:\$PATH\""

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

FLUTTER_ROOT="$HOME/.local/share/flutter"
FLUTTER_BIN="$FLUTTER_ROOT/bin"
if [[ ! -d "$FLUTTER_ROOT" ]]; then
    info "Cloning Flutter (stable channel)"
    git clone --depth 1 --branch stable https://github.com/flutter/flutter.git "$FLUTTER_ROOT"
else
    info "Updating Flutter"
    git -C "$FLUTTER_ROOT" fetch --depth 1 origin stable
    git -C "$FLUTTER_ROOT" reset --hard origin/stable
fi

export PATH="$FLUTTER_BIN:$CMDLINE_DIR/bin:$PATH"
ensure_profile_line "export PATH=\"$FLUTTER_BIN:\$PATH\""

"$FLUTTER_BIN/flutter" --version >/dev/null || warn "flutter --version encountered issues"

if command_exists snap && ! command_exists code; then
    info "Installing VS Code via snap"
    if [[ -n "$SUDO" ]]; then
        $SUDO snap install code --classic || warn "snap install code failed"
    else
        snap install code --classic || warn "snap install code failed"
    fi
fi

if command_exists snap && ! command_exists studio.sh; then
    info "Installing Android Studio via snap"
    if [[ -n "$SUDO" ]]; then
        $SUDO snap install android-studio --classic || warn "snap install android-studio failed"
    else
        snap install android-studio --classic || warn "snap install android-studio failed"
    fi
fi

if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
fi

if ! command_exists rustup; then
    info "Installing rustup"
    curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path --default-toolchain stable
    if [[ -f "$HOME/.cargo/env" ]]; then
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
    fi
fi

if command_exists rustup; then
    info "Updating Rust toolchains"
    rustup self update
    rustup toolchain install stable
    rustup default stable
    ok "Rust stable ready"
else
    warn "rustup not available; skipping Rust setup"
fi

if command_exists cargo; then
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
                "$HOME/.config/Code/extensions"
                "$HOME/.var/app/com.visualstudio.code/data/vscode/extensions"
                "$HOME/.var/app/com.visualstudio.code-oss/data/vscode/extensions"
                "$HOME/snap/code/current/.vscode/extensions"
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

ok "Setup wizard finished. Open a new shell to ensure PATH updates take effect."
info "Next steps:"
printf '  1) Launch Android Studio once and finish its setup wizard.\n'
printf '  2) Run: flutter doctor --android-licenses\n'
printf '  3) Verify everything with: flutter doctor\n'
