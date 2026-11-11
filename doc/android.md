# Android

How to set up a Windows machine to build feather apps for Android, and how the test app is built
and run on a device.

> **Status: in progress.** Android support is being added following
> [the Android plan](todo/android-plan.md). This page grows with each step of it, and holds only
> what has been done and checked. Sections not written yet are marked as such.

## Overview

A feather app on Android is a Rust shared library (`.so`) loaded by a small Java/Kotlin activity
(`GameActivity`). The pieces:

```
Rust sources ──cargo ndk──▶ lib<app>.so ─┐
build.rs ──▶ assets/ (SPIR-V, atlases,   ├──Gradle──▶ APK ──adb──▶ device
             scenes, meshes)             │
MainActivity.kt + AndroidManifest.xml ───┘
```

- **`cargo ndk`** cross-compiles the app's library for the device's CPU (`arm64-v8a`), using
  the Android NDK's linker.
- **`build.rs`** builds the assets on the host, as on Windows: shaders are compiled to SPIR-V
  with `glslc`, atlases packed, fonts rendered.
- **Gradle** packs the library, the assets and the activity into an APK and installs it.

The app's lifecycle on Android (the window coming and going) is described here once it is
implemented (plan step 5).

## Setup

Everything below is installed once per machine. Each tool has *install*, *configure* (where it
ends up, environment variables) and *check* (a command and what it should print).

Checked on: Windows 11 on ARM64 (`aarch64-pc-windows-msvc` host), November 2026.

What the build needs, at a glance:

| Tool                     | Version here           | Used for                                   |
|--------------------------|------------------------|--------------------------------------------|
| Android SDK Platform     | 33 (Android 13)        | compiling the activity against Android 13  |
| Android SDK Build-Tools  | 36.0.0 or newer        | packing the APK (AGP 9.5 needs 36.0.0)     |
| Android SDK Platform-Tools | `adb` 1.0.41 (37.0.1) | installing on the device, logs           |
| Android NDK              | 30.0.16248370 (r30)    | linking the Rust library for Android       |
| JDK                      | 25 (Android Studio's `jbr`) | running Gradle (9.6.0)                |
| Rust target              | `aarch64-linux-android` | compiling Rust for the device             |
| `cargo-ndk`              | 4.1.2                  | driving `cargo` with the NDK               |
| Vulkan SDK               | 1.4.304.0              | `glslc` (shaders), on the host only        |

### Android SDK

**Install.** The SDK comes with Android Studio. In Android Studio, open the SDK Manager
(*Settings → Languages & Frameworks → Android SDK*) and install:

- *SDK Platforms*: **Android 13 (API level 33)**;
- *SDK Tools*: **Android SDK Build-Tools**, **Android SDK Platform-Tools**,
  **Android SDK Command-line Tools**.

**Configure.** The SDK is installed in `%LOCALAPPDATA%\Android\Sdk`
(`C:\Users\<you>\AppData\Local\Android\Sdk`). No environment variable is needed: Gradle reads
the SDK's location from the Android project's `local.properties` (`sdk.dir=...`, written by
Android Studio, not committed), and `cargo ndk` finds this default location by itself.
`ANDROID_HOME` can point elsewhere if the SDK is installed in another place.

`adb` is in the SDK's `platform-tools` directory, which is not on the `PATH` by default. Either
add `%LOCALAPPDATA%\Android\Sdk\platform-tools` to the `PATH`, or call it by its full path.

**Check.**

```powershell
ls $env:LOCALAPPDATA\Android\Sdk\platforms     # android-33
ls $env:LOCALAPPDATA\Android\Sdk\build-tools   # 36.0.0 or newer
& $env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe version
```

```
Android Debug Bridge version 1.0.41
Version 37.0.1-15733141
```

### Android NDK

**Install.** In the SDK Manager, *SDK Tools*: **NDK (Side by side)**. Several versions can be
installed side by side.

**Configure.** NDKs are installed in `%LOCALAPPDATA%\Android\Sdk\ndk\<version>`. `cargo ndk`
uses the newest one there unless `ANDROID_NDK_HOME` names another; no environment variable is
needed.

The NDK for Windows exists only for x86-64 (`toolchains\llvm\prebuilt\windows-x86_64`). On an
ARM64 Windows machine it runs under Windows' x86-64 emulation: slower to link, but it works
(checked below).

**Check.** Which NDK `cargo ndk` picks:

```powershell
cargo ndk-env --target arm64-v8a
```

The paths it prints (`CC_aarch64_linux_android`, `AR_aarch64_linux_android`, ...) are in the
NDK it uses, e.g. `.../Android/sdk/ndk/30.0.16248370/toolchains/llvm/prebuilt/windows-x86_64/...`.

### JDK

Gradle runs on a JDK. The one bundled with Android Studio (its `jbr` directory, JetBrains
Runtime) is enough; there is nothing else to install.

Its version decides which Gradle can run: here it is **JDK 25**, which needs Gradle 9.1 or newer.
The Android project therefore uses Gradle 9.6.0 with Android Gradle plugin (AGP) 9.5.1.
GuitarPrompter's older Gradle 8.10.2 and AGP 8.8 run only on JDK 17 to 23. A newer Android Studio
may bundle a newer JDK: check Gradle's
[compatibility table](https://docs.gradle.org/current/userguide/compatibility.html) ("Support
for running Gradle") before relying on it.

**Install.** Nothing beyond Android Studio.

**Configure.** Point `JAVA_HOME` at Android Studio's `jbr`, so that `gradlew` finds it from the
command line (Android Studio itself finds it without this). For the current user, permanently:

```powershell
[Environment]::SetEnvironmentVariable("JAVA_HOME", "C:\Program Files\Android\Android Studio\jbr", "User")
```

Use the directory Android Studio is actually installed in: a reinstall can leave it in
`Android Studio1`, `Android Studio2`, ..., with the older directories empty but for
`uninstall.exe`. Open a new terminal afterwards: running ones keep the old environment.

**Check.**

```powershell
& "$env:JAVA_HOME\bin\java.exe" -version
```

```
openjdk version "25.0.4" 2026-07-21
...
```

### Rust target and `cargo-ndk`

**Install.**

```powershell
rustup target add aarch64-linux-android
cargo install cargo-ndk
```

**Configure.** Nothing. `cargo-ndk` is a `cargo` subcommand: `cargo ndk -t <abi> <cargo command>`
runs the cargo command for the Android target matching the ABI (`arm64-v8a` →
`aarch64-linux-android`), with the NDK's tools as linker.

**Check.**

```powershell
rustup target list --installed   # includes aarch64-linux-android
cargo ndk --version              # cargo-ndk 4.1.2
```

Then build a throwaway library for Android, in an empty directory outside the repository:

```powershell
cargo new --lib ndkcheck
cd ndkcheck
Add-Content Cargo.toml "`n[lib]`ncrate-type = [`"cdylib`"]"
cargo ndk -t arm64-v8a build
```

It ends with `Finished`, and builds
`target\aarch64-linux-android\debug\libndkcheck.so`, an ARM64 Android library (`file` from Git
Bash shows `ELF 64-bit LSB shared object, ARM aarch64`).

### Vulkan SDK

The Vulkan SDK is needed on the host as for Windows builds (see the README's *Requirements*):
`build.rs` compiles the shaders with its `glslc`, and SPIR-V is the same on every platform.
Nothing from it goes to the device: Android has its own Vulkan driver, and the validation layers
are not installed on devices (see *Logs and debugging*).

**Check.**

```powershell
glslc --version
```

```
shaderc v2023.8 v2024.4
...
Target: SPIR-V 1.0
```

### The device

*Not written yet (plan step 9): developer options, USB debugging, `adb devices`, checking
Vulkan support.*

## Building and running

*Not written yet (plan steps 2, 8 and 9).*

## Logs and debugging

*Not written yet (plan steps 4, 6, 8 and 9).*

## Making an app run on Android

*Not written yet (plan steps 3, 7, 8 and 9).*

## Troubleshooting

*Not written yet: problems met during the plan's steps, with their symptoms and fixes.*
