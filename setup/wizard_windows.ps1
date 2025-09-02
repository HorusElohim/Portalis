<#
.SYNOPSIS
  Portalis Windows Dev Environment Wizard
.DESCRIPTION
  Idempotent PowerShell wizard that installs and configures:
    - winget-managed: Git, VS Code, Flutter, Android Studio, OpenJDK 17, Rust (rustup), VS 2022 Build Tools
    - Android SDK base, platform-tools, build-tools, emulator; accepts licenses
    - VS Code extensions (Flutter, Dart, Rust Analyzer, TOML, EditorConfig)
    - Rust tools: flutter_rust_bridge_codegen, gitmoji-rs
    - Environment variables: JAVA_HOME, ANDROID_SDK_ROOT, PATH
    - Validates via flutter doctor / rustc / cargo
.NOTES
  Run in an elevated PowerShell. Safe to re-run.
#>

# region Utilities -------------------------------------------------------------

$ErrorActionPreference = 'Stop'

function Write-Info($msg) { Write-Host "[INFO ] $msg" -ForegroundColor Cyan }
function Write-Ok($msg) { Write-Host "[ OK  ] $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "[WARN ] $msg" -ForegroundColor Yellow }
function Write-Err($msg) { Write-Host "[ERROR] $msg" -ForegroundColor Red }

function Require-Admin {
    $currentIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($currentIdentity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        Write-Err "Please run PowerShell as Administrator."
        exit 1
    }
}

function Test-Cmd($name) {
    $null -ne (Get-Command $name -ErrorAction SilentlyContinue)
}

function Ensure-InPath([string]$dir) {
    if (-not (Test-Path $dir)) { return }
    $current = [Environment]::GetEnvironmentVariable("Path", "Machine")
    if (-not $current.Split(';') -contains $dir) {
        [Environment]::SetEnvironmentVariable("Path", "$current;$dir", "Machine")
        Write-Ok "Added to PATH (machine): $dir"
    }
}

function Set-EnvMachine([string]$name, [string]$value) {
    $existing = [Environment]::GetEnvironmentVariable($name, "Machine")
    if ($existing -ne $value -and $value) {
        [Environment]::SetEnvironmentVariable($name, $value, "Machine")
        Write-Ok "Set $name = $value"
    }
}

function Confirm-Yes([string]$prompt, [bool]$default = $true) {
    $suffix = if ($default) { "[Y/n]" } else { "[y/N]" }
    $answer = Read-Host "$prompt $suffix"
    if ([string]::IsNullOrWhiteSpace($answer)) { return $default }
    return @("y", "yes") -contains $answer.ToLower()
}

# endregion Utilities ----------------------------------------------------------

# region System & Arch ---------------------------------------------------------

Require-Admin

$IsArm64 = ([Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE") -eq "ARM64") -or `
([Environment]::Is64BitOperatingSystem -and `
    (Get-CimInstance Win32_Processor).Architecture -eq 12)

Write-Info ("Detected architecture: " + ($(if ($IsArm64) { "ARM64" }else { "x64" })))

# endregion -------------------------------------------------------------------

# region Winget ----------------------------------------------------------------

if (-not (Test-Cmd winget)) {
    Write-Warn "winget not found. Please install App Installer from Microsoft Store and re-run."
    Start-Process "ms-windows-store://pdp/?ProductId=9NBLGGH4NNS1"
    exit 1
}

function Install-IfMissingWinget($id, $name) {
    $installed = winget list --id $id --accept-source-agreements | Out-String
    if ($installed -match $id) {
        Write-Ok "$name already installed."
    }
    else {
        Write-Info "Installing $name..."
        winget install --id $id --accept-package-agreements --accept-source-agreements -h | Out-Null
        Write-Ok "$name installed."
    }
}

# endregion -------------------------------------------------------------------

# region Core Packages ---------------------------------------------------------

# Git
Install-IfMissingWinget "Git.Git" "Git"

# VS Code
Install-IfMissingWinget "Microsoft.VisualStudioCode" "Visual Studio Code"

# Rust (rustup)
Install-IfMissingWinget "Rustlang.Rustup" "Rustup"

# C++ Build Tools (for native Rust crates)
if (Confirm-Yes "Install Visual Studio 2022 Build Tools (C++ toolchain)?" $true) {
    $vsId = "Microsoft.VisualStudio.2022.BuildTools"
    $installed = winget list --id $vsId | Out-String
    if ($installed -match $vsId) {
        Write-Ok "VS Build Tools already installed."
    }
    else {
        Write-Info "Installing VS Build Tools (this may take a while)..."
        # Minimal C++ workload + Spectre libs + CMake + Windows SDK
        winget install --id $vsId `
            --accept-package-agreements --accept-source-agreements -h `
            --override '--quiet --wait --norestart --nocache --installPath "C:\BuildTools" --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --add Microsoft.VisualStudio.Component.Windows10SDK.19041' | Out-Null
        Write-Ok "VS Build Tools installed."
    }
}

# Java 17 (OpenJDK) for Android builds
$jdkId = if ($IsArm64) { "Microsoft.OpenJDK.17" } else { "Microsoft.OpenJDK.17" }
Install-IfMissingWinget $jdkId "OpenJDK 17"

# Flutter & Android Studio
Install-IfMissingWinget "Google.Flutter"       "Flutter SDK"
Install-IfMissingWinget "Google.AndroidStudio" "Android Studio"

# endregion -------------------------------------------------------------------

# region Paths & Environment ---------------------------------------------------

# Try to resolve JAVA_HOME from Microsoft OpenJDK
try {
    $jdkRoot = (Get-ChildItem "C:\Program Files\Microsoft\jdk-17*" -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1).FullName
    if ($jdkRoot) { Set-EnvMachine "JAVA_HOME" $jdkRoot; Ensure-InPath "$jdkRoot\bin" }
}
catch { Write-Warn "Could not auto-detect JAVA_HOME. Ensure JDK 17 is installed." }

# Android SDK common locations
$androidSdk = $env:ANDROID_SDK_ROOT
if (-not $androidSdk -or -not (Test-Path $androidSdk)) {
    $candidates = @(
        "$env:LOCALAPPDATA\Android\Sdk",
        "$env:APPDATA\Local\Android\Sdk",
        "C:\Android\Sdk"
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { $androidSdk = $c; break }
    }
}

if (-not $androidSdk) {
    # If not found, propose default
    $androidSdk = "$env:LOCALAPPDATA\Android\Sdk"
}

Set-EnvMachine "ANDROID_SDK_ROOT" $androidSdk
Ensure-InPath "$androidSdk\platform-tools"
Ensure-InPath "$androidSdk\emulator"
Ensure-InPath "$androidSdk\cmdline-tools\latest\bin"

# Flutter on PATH (winget installs under Program Files)
$candFlutter = @(
    "$env:ProgramFiles\flutter\bin",
    "$env:LOCALAPPDATA\flutter\bin"
) | Where-Object { Test-Path $_ }

foreach ($p in $candFlutter) { Ensure-InPath $p }

# Rust & Cargo PATH (user scope, but ensure for machine PATH too for CI shells)
$cargoBin = "$env:USERPROFILE\.cargo\bin"
if (Test-Path $cargoBin) { Ensure-InPath $cargoBin }

# endregion -------------------------------------------------------------------

# region Android SDK packages & licenses --------------------------------------

function Get-SdkManager {
    $paths = @(
        "$androidSdk\cmdline-tools\latest\bin\sdkmanager.bat",
        "$androidSdk\cmdline-tools\bin\sdkmanager.bat",
        "$androidSdk\tools\bin\sdkmanager.bat"
    )
    foreach ($p in $paths) {
        if (Test-Path $p) { return $p }
    }
    return $null
}

function Ensure-AndroidCmdlineTools {
    $sdkMgr = Get-SdkManager
    if ($sdkMgr) { return $sdkMgr }
    Write-Info "Installing Android cmdline-tools..."
    # Install via sdkmanager requires cmdline-tools; if missing, install from winget-managed Android Studio first:
    $studioBase = "$env:LOCALAPPDATA\Android\Sdk\cmdline-tools"
    if (-not (Test-Path $studioBase)) { New-Item -ItemType Directory -Force -Path $studioBase | Out-Null }
    # Android Studio usually ships them on first update; prompt user to open once if not found
    Write-Warn "If sdkmanager still cannot be found, launch Android Studio once: More Actions → SDK Manager → install 'Android SDK Command-line Tools (latest)'."
    return (Get-SdkManager)
}

# Override with auto-download implementation to ensure sdkmanager is available
function Ensure-AndroidCmdlineTools {
    $sdkMgr = Get-SdkManager
    if ($sdkMgr) { return $sdkMgr }
    Write-Info "Android cmdline-tools not found. Attempting direct download from Google..."
    try {
        if (-not (Test-Path $androidSdk)) { New-Item -ItemType Directory -Force -Path $androidSdk | Out-Null }
        $baseRepo = "https://dl.google.com/android/repository/"
        $repoXml  = "${baseRepo}repository2-3.xml"
        $tmpDir = Join-Path $env:TEMP ("android-tools-" + [Guid]::NewGuid())
        New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null
        $xmlPath = Join-Path $tmpDir "repository2-3.xml"
        Invoke-WebRequest -UseBasicParsing -Uri $repoXml -OutFile $xmlPath | Out-Null
        $xmlContent = Get-Content $xmlPath -Raw
        $match = [regex]::Match($xmlContent, "commandlinetools-win-\d+_latest\.zip")
        if (-not $match.Success) { throw "Could not determine cmdline-tools latest zip from repository index." }
        $zipName = $match.Value
        $zipUrl  = "$baseRepo$zipName"
        $zipPath = Join-Path $tmpDir $zipName
        Write-Info "Downloading: $zipUrl"
        Invoke-WebRequest -UseBasicParsing -Uri $zipUrl -OutFile $zipPath | Out-Null
        $extractDir = Join-Path $tmpDir "extract"
        Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force
        $srcTools = Join-Path $extractDir "cmdline-tools"
        if (-not (Test-Path $srcTools)) { throw "Downloaded archive missing 'cmdline-tools' directory." }
        $dstLatest = Join-Path $androidSdk "cmdline-tools\latest"
        New-Item -ItemType Directory -Force -Path $dstLatest | Out-Null
        Copy-Item -Path (Join-Path $srcTools '*') -Destination $dstLatest -Recurse -Force
        Ensure-InPath (Join-Path $dstLatest "bin")
        Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
        $sdkMgr = Get-SdkManager
        if ($sdkMgr) { Write-Ok "Android cmdline-tools installed."; return $sdkMgr }
        throw "cmdline-tools installation did not yield sdkmanager."
    }
    catch {
        Write-Warn ("Auto-install of cmdline-tools failed: " + $_)
        Write-Warn "Open Android Studio → More Actions → SDK Manager → install 'Android SDK Command-line Tools (latest)'."
        return (Get-SdkManager)
    }
}

if (Confirm-Yes "Install Android SDK packages (API 34) & accept licenses?" $true) {
    $sdkmanager = Ensure-AndroidCmdlineTools
    if ($sdkmanager) {
        $env:JAVA_HOME = [Environment]::GetEnvironmentVariable("JAVA_HOME", "Machine")
        $env:ANDROID_SDK_ROOT = [Environment]::GetEnvironmentVariable("ANDROID_SDK_ROOT", "Machine")

        & $sdkmanager --sdk_root="$androidSdk" "platform-tools" "platforms;android-34" "build-tools;34.0.0" "emulator" | Out-Null
        Write-Ok "Android SDK components installed/updated."

        # Accept licenses (feed "y" automatically). Create input file first, then run once.
        $licFile = Join-Path $env:TEMP ("sdk-licenses-" + [Guid]::NewGuid() + ".txt")
        "y`n" * 200 | Set-Content -Path $licFile -Encoding ascii
        Start-Process -FilePath $sdkmanager `
            -ArgumentList "--sdk_root=$androidSdk", "--licenses" `
            -RedirectStandardInput $licFile `
            -NoNewWindow -Wait | Out-Null
        Remove-Item $licFile -Force -ErrorAction SilentlyContinue
        Write-Ok "Android licenses accepted."
    }
    else {
        Write-Warn "sdkmanager not found. Open Android Studio → SDK Manager → install 'Android SDK Command-line Tools (latest)'."
    }
}

# endregion -------------------------------------------------------------------

# region VS Code Extensions ----------------------------------------------------

if (Test-Cmd code) {
    function Install-Ext($ext) {
        $list = code --list-extensions | Out-String
        if ($list -match [Regex]::Escape($ext)) {
            Write-Ok "VS Code extension already present: $ext"
        }
        else {
            Write-Info "Installing VS Code extension: $ext"
            code --install-extension $ext --force | Out-Null
        }
    }

    Install-Ext "Dart-Code.dart-code"
    Install-Ext "Dart-Code.flutter"
    Install-Ext "rust-lang.rust-analyzer"
    Install-Ext "tamasfe.even-better-toml"
    Install-Ext "EditorConfig.EditorConfig"
}
else {
    Write-Warn "VS Code CLI not found on PATH yet. Open VS Code once, then re-run to auto-install extensions."
}

# endregion -------------------------------------------------------------------

# region Rust toolchain & crates ----------------------------------------------

# Ensure default toolchain
if (Test-Cmd rustup) {
    Write-Info "Updating Rust toolchains…"
    rustup self update | Out-Null
    rustup toolchain install stable | Out-Null
    rustup default stable | Out-Null
    Write-Ok "Rust stable ready."
}
else {
    Write-Warn "rustup not on PATH yet. Open a new terminal or re-run after a logoff."
}

# flutter_rust_bridge_codegen
if (Test-Cmd cargo) {
    if (-not (Test-Cmd flutter_rust_bridge_codegen)) {
        Write-Info "Installing flutter_rust_bridge_codegen…"
        cargo install flutter_rust_bridge_codegen | Out-Null
        Write-Ok "Installed flutter_rust_bridge_codegen."
    }
    else {
        Write-Ok "flutter_rust_bridge_codegen already installed."
    }

    # gitmoji-rs (optional)
    if (Confirm-Yes "Install gitmoji-rs (emoji commit assistant)?" $true) {
        if (-not (Test-Cmd gitmoji)) {
            Write-Info "Installing gitmoji-rs…"
            cargo install gitmoji-rs | Out-Null
            Write-Ok "Installed gitmoji-rs."
        }
        else {
            Write-Ok "gitmoji-rs already installed."
        }
    }
}
else {
    Write-Warn "cargo not on PATH yet. Open a new terminal or re-run after a logoff."
}

# endregion -------------------------------------------------------------------

# region Optional: Create/Update a Portalis Flutter app -----------------------

if (Confirm-Yes "Create/upgrade a local Flutter app named 'Portalis' in the current directory?" $false) {
    if (-not (Test-Cmd flutter)) {
        Write-Warn "Flutter CLI not found. Open a new terminal or re-run after ensuring flutter\bin is in PATH."
    }
    else {
        $proj = Join-Path (Get-Location) "Portalis"
        if (Test-Path $proj) {
            Write-Info "Project exists. Running 'flutter pub get'…"
            Push-Location $proj
            flutter pub get
            Pop-Location
        }
        else {
            Write-Info "Creating Flutter app 'Portalis'…"
            flutter create Portalis
        }
        Write-Ok "Portalis Flutter app ready: $proj"
    }
}

# endregion -------------------------------------------------------------------

# region Diagnostics -----------------------------------------------------------

Write-Info "Diagnostics:"
try {
    if (Test-Cmd flutter) {
        flutter --version
        flutter doctor
    }
    else {
        Write-Warn "Flutter CLI not yet on PATH (open a new terminal if you just installed it)."
    }
}
catch { Write-Warn "flutter doctor encountered issues. Open Android Studio, ensure SDK + licenses; then re-run." }

try { rustc --version } catch { Write-Warn "rustc not found in current shell." }
try { cargo --version } catch { Write-Warn "cargo not found in current shell." }

Write-Ok "Setup wizard finished. If some tools weren't detected, open a NEW PowerShell and re-run for PATH to refresh."

# Post-setup guidance
Write-Info "Next steps:"
Write-Host "  1) Launch Android Studio once → complete the initial Setup Wizard." -ForegroundColor Cyan
Write-Host "     Then open SDK Manager and ensure 'Android SDK Command-line Tools (latest)' is installed (the wizard tried to install it automatically)." -ForegroundColor Cyan
Write-Host "  2) In a new terminal, run: flutter doctor --android-licenses" -ForegroundColor Cyan
Write-Host "  3) In VS Code, ensure Flutter and Dart extensions are enabled (the wizard installed them if 'code' CLI was available)." -ForegroundColor Cyan
Write-Host "  4) Verify with: flutter doctor" -ForegroundColor Cyan


# Expected flutter doctor output (for reference)
Write-Host "Expected final 'flutter doctor' result:" -ForegroundColor Cyan
Write-Host " Flutter (Channel stable, 3.35.2, on Microsoft Windows [Version 10.0.26100.5074], locale en-150)" -ForegroundColor Green
Write-Host " Windows Version (Windows 11 or higher, 24H2, 2009)" -ForegroundColor Green
Write-Host " Android toolchain - develop for Android devices (Android SDK version 36.1.0-rc1)" -ForegroundColor Green
Write-Host " Chrome - develop for the web" -ForegroundColor Green
Write-Host " Visual Studio - develop Windows apps (Visual Studio Build Tools 2022 17.14.13 (August 2025))" -ForegroundColor Green
Write-Host " Android Studio (version 2025.1.3)" -ForegroundColor Green
Write-Host " VS Code (version 1.103.2)" -ForegroundColor Green
Write-Host " Connected device (3 available)" -ForegroundColor Green
Write-Host " Network resources" -ForegroundColor Green

# endregion -------------------------------------------------------------------
