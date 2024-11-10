
# Setup Guide for Portalis

Welcome to the **Portalis** project! This guide will help you set up your development environment. We’ll be installing the necessary tools and ensuring everything is configured properly so you can start working on the project.

## Prerequisites

Before you start, make sure your system meets the following requirements:

- **Operating System**: Windows, macOS, or Linux
- **Disk Space**: Minimum 10 GB of free space
- **Internet Connection**: Required for downloading tools and dependencies

---

## 1. Install VSCode

VSCode (Visual Studio Code) is a versatile code editor that supports both Flutter and Rust. Download and install it by following these steps:

1. Go to the [VSCode website](https://code.visualstudio.com/).
2. Download the installer for your operating system.
3. Run the installer and follow the on-screen instructions.
4. Once installed, open VSCode to complete the setup.

---

## 2. Install Flutter VSCode Extension

The Flutter VSCode extension enables Flutter support within VSCode, allowing you to run and debug your Flutter applications directly from the editor.

1. Open VSCode.
2. Navigate to the **Extensions** tab (or press `Ctrl+Shift+X` on Windows/Linux, `Cmd+Shift+X` on macOS).
3. Search for "Flutter" and select the **Flutter** extension by `Dart Code`.
4. Click **Install**.
5. When prompted, also install the **Dart** extension (a dependency of Flutter).

---

## 3. Create a New Flutter Project

With the Flutter extension installed, let’s create a new Flutter project:

1. Open the **Command Palette** (press `Ctrl+Shift+P` on Windows/Linux or `Cmd+Shift+P` on macOS).
2. Type `Flutter: New Project` and select it from the list.
3. Choose a name for your project (e.g., `Portalis`) and select a location to save it.
4. Once the setup is complete, you’ll have a new Flutter project structure in your chosen directory.

---

## 4. Set Up Flutter Through the VSCode Extension

To ensure that Flutter is correctly set up within VSCode:

1. Open the Command Palette again (`Ctrl+Shift+P` or `Cmd+Shift+P`).
2. Type `Flutter: Open Android Emulator` and select it to start the Android emulator (if you plan to run on an Android device).
3. Run `flutter doctor` in the VSCode terminal (open with `Ctrl+`` or `Cmd+``).
4. Follow any instructions provided by `flutter doctor` to resolve issues.

---

## 5. Install Android Studio and SDK

Android Studio is required to build Flutter apps for Android. It also includes the Android SDK and command-line tools needed to run Android emulators.

1. Download Android Studio from the [official website](https://developer.android.com/studio).
2. Run the installer and follow the on-screen instructions.
3. After installation, open Android Studio and go to **SDK Manager**:
   - **Configure** > **SDK Manager**
   - Make sure the **Android SDK** and **SDK Command-line Tools** are installed.

4. Open a terminal or VSCode terminal and run:

   ```bash
   flutter doctor --android-licenses
   ```

   This command accepts the necessary licenses for using the Android SDK.

5. Re-run `flutter doctor` to confirm that Android Studio is correctly set up.

---

## 6. Install Rust

Rust is a system programming language that we will use for the backend portion of this project. Install Rust by following these steps:

1. Go to the [Rust website](https://www.rust-lang.org/tools/install).
2. Run the installation command provided for your operating system. For Unix-based systems (macOS/Linux), you can use the following command:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. Follow the on-screen instructions to complete the installation.

---

## 7. Add Rust to Your System Path

After installing Rust, add it to your system `PATH` so that it can be accessed from the terminal.

1. Open a new terminal window and run:

   ```bash
   source $HOME/.cargo/env
   ```

2. Verify the installation by running:

   ```bash
   rustc --version
   ```

   This command should display the installed Rust version if everything is set up correctly.

---

## 8. Install `flutter_rust_bridge_codegen`

`flutter_rust_bridge_codegen` is a tool that generates bindings to facilitate communication between Flutter (Dart) and Rust.

1. Install the tool by running:

   ```bash
   cargo install flutter_rust_bridge_codegen
   ```

2. Confirm the installation by running:

   ```bash
   flutter_rust_bridge_codegen --version
   ```

   You should see the version number, confirming the installation was successful.

---

## 9. Install gitmoji-rs for Conventional Emoji-based Git Commits

`gitmoji-rs` is a command-line tool that provides emojis for commit messages, following the Gitmoji standard. This helps make commit messages visually consistent and easy to understand.

1. Install by running:

   ```bash 
   cargo install gitmoji-rs
   ```

2. Initialize  

   ```bash
   gitmoji init 
   ```

3. Commit 
   
   ```bash
   gitmoji commit 
   gitmoji -c
   ```

---

## Troubleshooting

If you encounter issues during setup, consider:

- **Checking for Updates**: Make sure all tools and extensions are up-to-date.
- **Re-running `flutter doctor`**: This command can help identify missing dependencies.
- **Referencing Online Documentation**:
  - [Flutter Setup Guide](https://flutter.dev/docs/get-started/install)
  - [Rust Installation Guide](https://www.rust-lang.org/tools/install)

---

This completes the initial setup for **Portalis**. Happy coding!
