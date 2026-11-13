# Android

How to set up a Windows machine to build feather apps for Android, and how the test app is built
and run on a device.

> Everything here was done and checked on one machine (Windows 11 on ARM64) and one phone (a
> Galaxy S25), following [the Android plan](archive/done-android-plan.md); what is left for later is in
> [todo/android-plan.md](todo/android-plan.md).

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

### Lifecycle

On Windows a window exists from start to end. On Android the activity's window (and so the Vulkan
surface the engine draws into) is destroyed whenever the app goes to the background or the screen
is locked, and made again when it comes back, while the process and everything in memory stay.
winit reports this with two events, sent on every platform, and the engine
(`App::run`) follows them:

```
start ──▶ Resumed (first) ──▶ engine created: Vulkan device, surface, swapchain;
                              app.on_create(); app.on_resized()
               │
               ▼
          drawing frames ◀──────────────────────────────┐
               │                                        │
               ▼                                        │
          Suspended ──▶ surface and swapchain released; │
                        nothing drawn                   │
               │                                        │
               ▼                                        │
          Resumed ────▶ surface and swapchain made      │
                        again; app.on_resized() ────────┘

window closed (Windows) / back (Android: request_exit) ──▶ exiting ──▶ engine and app destroyed
```

What an app can count on across `Suspended` / `Resumed`:

- **Kept**: the app itself and its state, its scenes, and the GPU copies of their meshes and
  atlases (the Vulkan device stays). Nothing has to be reloaded.
- **Rebuilt by the engine**: the surface, the swapchain and what depends on them (pipelines,
  framebuffers). This is the same work as when the window is resized.
- **Told to the app**: `on_resized` with the window's size after each `Resumed`, since it may
  have changed meanwhile.
- **Not drawn**: no frames between `Suspended` and `Resumed`; `on_update` and `on_render` aren't
  called. Scene animations follow the clock, so they jump ahead by the time spent in the
  background.

(The C++ engine on Android loses its whole OpenGL context and reloads every GPU resource on
resume. With Vulkan only the surface goes.)

On Windows the same path can be exercised without a device: in a debug build, **F9** suspends and
resumes at once. Running a demo, pressing F9 a few times and switching demos in between should
show no validation errors, and the log shows each step:

```
INFO  feather::app > Suspended: releasing the surface and the swapchain.
INFO  feather::app > Resumed: recreating the surface and the swapchain.
```

## Setup

Everything below is installed once per machine. Each tool has *install*, *configure* (where it
ends up, environment variables) and *check* (a command and what it should print).

Checked on: Windows 11 on ARM64 (`aarch64-pc-windows-msvc` host), November 2026.

What the build needs, at a glance:

| Tool                     | Version here           | Used for                                   |
|--------------------------|------------------------|--------------------------------------------|
| Android SDK Platform     | 37 (Android 17)        | compiling the activity against Android 17  |
| Android SDK Build-Tools  | 36.0.0 or newer        | packing the APK (AGP 9.5 needs 36.0.0)     |
| Android SDK Platform-Tools | `adb` 1.0.41 (37.0.1) | installing on the device, logs           |
| Android NDK              | 30.0.16248370 (r30)    | linking the Rust library for Android       |
| JDK                      | 25 (Android Studio's `jbr`) | running Gradle (9.9.1)                |
| Rust target              | `aarch64-linux-android` | compiling Rust for the device             |
| `cargo-ndk`              | 4.1.2                  | driving `cargo` with the NDK               |
| Vulkan SDK               | 1.4.304.0              | `glslc` (shaders), on the host only        |

### Android SDK

**Install.** The SDK comes with Android Studio. In Android Studio, open the SDK Manager
(*Settings → Languages & Frameworks → Android SDK*) and install:

- *SDK Platforms*: **Android 17 (API level 37)** (the project's `compileSdk`; if it is missing,
  the first Gradle build downloads it, as it did here, provided the SDK's licenses were accepted
  in the SDK Manager);
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
ls $env:LOCALAPPDATA\Android\Sdk\platforms     # android-37.0 (or another android-37.x)
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
The Android project therefore uses Gradle 9.9.1 with Android Gradle plugin (AGP) 9.5.1 (which
needs Gradle 9.6.0 or newer).
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

A real phone, connected by USB. (The emulator's Vulkan support varies; it isn't used here.)

Checked with: Samsung Galaxy S25 (SM-S931B), Android 17 (API 37), Snapdragon 8 Elite (SM8750)
with an Adreno 830 GPU, Vulkan 1.3, screen 1080×2340.

**Install.** On the phone, turn on USB debugging (the steps are Samsung's; other phones name
things slightly differently):

1. *Settings → About phone → Software information*: tap **Build number** seven times (and
   confirm with the PIN). *Developer options* appear in *Settings*.
2. *Settings → Developer options*: turn on **USB debugging**.
3. Connect the phone (or unplug and reconnect it). The phone asks **Allow USB debugging?** with
   the computer's key fingerprint: tick *Always allow from this computer* and tap **Allow**.

**Configure.** Nothing on the computer: Windows' own driver handles `adb` for this phone.

Without USB debugging, Windows sees the phone only as a media device (it shows in Explorer by its
name) and `adb devices` lists nothing. In *Device Manager* (or `Get-PnpDevice` in PowerShell),
the phone's USB device then has only a portable device (MTP) and a serial (modem) interface; with
USB debugging on, it gains an ADB interface.

**Check.**

```powershell
adb devices -l
```

```
List of devices attached
RFCY50LYQGM            device product:pa1qxeea model:SM_S931B device:pa1q transport_id:1
```

`device` in the second column means ready; `unauthorized` means the *Allow USB debugging?*
prompt on the phone wasn't accepted yet.

Then what the phone is, and that it has Vulkan:

```powershell
adb shell getprop ro.build.version.sdk       # API level: 37 (Android 17); the app needs 30 or more
adb shell getprop ro.product.cpu.abilist     # arm64-v8a: what the app is built for
adb shell pm list features | Select-String vulkan
```

```
feature:android.hardware.vulkan.compute
feature:android.hardware.vulkan.level=1
feature:android.hardware.vulkan.version=4206592
feature:android.software.vulkan.deqp.level=132645633
```

`vulkan.version` is the Vulkan version packed as a number: `4206592` is `0x403000`, Vulkan 1.3.
The engine needs 1.0 (`4194304`, `0x400000`) or more.

## Building and running

### Building the APK and running it

The test app's Android project is in `apps\testapp\android`. Once, before the first build, write
its `local.properties` (where the SDK is; not committed, as it differs per machine; Android
Studio writes it too when it opens the project):

```powershell
cd apps\testapp\android
Set-Content local.properties "sdk.dir=$($env:LOCALAPPDATA -replace '\\','\\' -replace ':','\:')\\Android\\Sdk" -Encoding ascii
Get-Content local.properties    # sdk.dir=C\:\\Users\\<you>\\AppData\\Local\\Android\\Sdk
```

Then, with the phone connected (see *The device*):

```powershell
.\gradlew.bat installDebug
adb shell am start -n com.kashikume.feather.testapp/.MainActivity
```

`installDebug` builds the APK and installs it on the phone (`assembleDebug` only builds it, into
`app\build\outputs\apk\debug\app-debug.apk`). `am start` starts it; tapping *Feather test app* on
the phone does the same.

- The first build downloads Gradle 9.9.1 (the wrapper checks its SHA-256), the Android Gradle
  plugin and the AndroidX libraries, and builds every Rust dependency for Android: about three
  minutes here. Later builds take seconds when only a little changed.
- Gradle runs `cargo ndk` itself (`cargoNdkDebug`; see *The Gradle project*). It builds Rust
  **optimized** (`--release`) even for the debug APK: unoptimized Rust is too slow on a phone.
  For a Rust debug build: `.\gradlew.bat installDebug -Pfeather.rustProfile=debug`.
- The debug APK is signed with Android's debug key, created on first use in
  `%USERPROFILE%\.android\debug.keystore`. A release APK needs a key of its own (not set up yet).

The build ends with two notices that need no action:

- `Deprecated Gradle features were used in this build, making it incompatible with Gradle 10.`
  (`--warning-mode all` shows it is `Configuration.setVisible`, called by the Android Gradle
  plugin itself, not by this project's scripts; the plugin's next versions are expected to
  drop it.)
- `Consider enabling configuration cache`: a speed-up not set up yet.

The APK's contents take about 21 MB: in a debug APK, the AndroidX libraries' unshrunk Java code
(`classes.dex`, 8.6 MB); the PNG atlases (7.5 MB, stored as they are); the Rust library (2.7 MB
once Gradle strips its symbols); and the meshes, 9.8 MB of uncompressed `.fmesh` files that the
APK deflates to 1.1 MB (as OBJ files they were 32 MB, 6 MB deflated; see
[fmesh-format.md](fmesh-format.md)). After incremental builds the file itself can be larger
than its contents (28.9 MB on 2026-11-12).

### Checking that the code compiles for Android

From the workspace root:

```powershell
cargo ndk -t arm64-v8a check -p feather -p feather-ui
```

It ends with `Finished`. This is the quick check that a change didn't break the Android build,
without a device or Gradle.

Always go through `cargo ndk`: a plain `cargo check --target aarch64-linux-android` fails (see
*Troubleshooting*), because building for Android compiles some C++ (GameActivity's native part,
inside the `android-activity` crate) with the NDK's compiler, which only `cargo ndk` sets up.

Android builds go to `target\aarch64-linux-android\`, next to the Windows ones in
`target\debug\` and `target\release\`, so switching between the two doesn't rebuild everything.

### Building the app's library alone

The APK build (Gradle) runs this itself; on its own it is a quicker way to see that the whole app
builds for Android:

```powershell
cargo ndk -t arm64-v8a build -p testapp --lib
```

`--lib` builds only the library (the `testapp` binary is for Windows). The first build takes a
few minutes (every dependency is built for Android); later ones only rebuild what changed. It
produces, under `target\aarch64-linux-android\debug\`:

- `libfeather_testapp.so`: the app, about 65 MB in a debug build (debug information; release
  builds are much smaller);
- `assets\`: the assets, built by `build.rs` exactly as for Windows (149 files for the test app
  here).

Add `--release` for a release build (in `...\release\`).

To look inside the library, the NDK has LLVM's tools (in
`%LOCALAPPDATA%\Android\Sdk\ndk\<version>\toolchains\llvm\prebuilt\windows-x86_64\bin`):

```powershell
llvm-readelf -d target\aarch64-linux-android\debug\libfeather_testapp.so          # NEEDED: the libraries it needs
llvm-nm -D --defined-only target\aarch64-linux-android\debug\libfeather_testapp.so # its exported symbols
```

It needs only Android's system libraries (`liblog.so`, `libandroid.so`, `libdl.so`, `libm.so`,
`libc.so`): GameActivity's C++ code is linked with the C++ runtime statically, so no
`libc++_shared.so` has to go into the APK. It exports `android_main` and GameActivity's entry
points (`GameActivity_onCreate`, `Java_com_google_androidgamesdk_GameActivity_...`), which the
Java side calls.

### What runs on a phone

Checked on the Galaxy S25 (see *The device*), with the debug APK (Rust built optimized):

- All 23 demos of the test app draw as on Windows, with the HUD; the HUD's buttons switch demos
  by touch.
- Frame rate: one frame per display refresh (120 fps on this phone) for every demo but the two
  City demos (25–45 fps) and Scrolling (about 65 fps).
- Going to the background and back keeps the demo where it was; back leaves the app.

Not checked yet: turning the phone to the other landscape side, and locking and unlocking the
screen.

## Logs and debugging

### Reading the log

The app's log goes to the phone's log (logcat), which `adb` shows:

```powershell
adb logcat -c                       # clear what is there
adb logcat -s feather:V GameActivity:V AndroidRuntime:E libc:F DEBUG:F
```

- `feather`: the engine's and the app's log (the tag set in `android_main`);
- `GameActivity`: the activity loading the library and its lifecycle;
- `AndroidRuntime`, `libc`, `DEBUG`: crashes (Java exceptions, native signals, and the native
  crash report with its backtrace).

`adb logcat -d` prints what is there and stops, instead of following. A start of the test app
looks like this (on the Galaxy S25):

```
I GameActivity: Looking for library libfeather_testapp.so
I GameActivity: Found library libfeather_testapp.so. Loading...
I feather : feather_testapp::android: Starting the feather test app for Android.
I feather : feather_testapp::testapp::testapp: Demo 0: Viking room
I feather : feather::physicaldevice: Selected physical device (`Adreno (TM) 830`).
I feather : feather::swapchain: Surface: current extent 2340x1080, image extent 1x1 to 4096x4096, window 2340x1080, current transform ROTATE_90, supported transforms IDENTITY | ROTATE_90 | ..., composite alpha INHERIT, 4 to 64 images
I feather : feather::swapchain: Swapchain: R8G8B8A8_SRGB SRGB_NONLINEAR, 2340x1080, FIFO, transform IDENTITY, composite alpha INHERIT
```

and back, leaving the app:

```
I feather : feather::app: Exit requested.
I feather : feather_testapp::android: The feather test app has ended.
I Zygote  : Process 31310 exited cleanly (0)
```

A screenshot of the phone, into a file (from Git Bash: PowerShell's `>` would corrupt the
image):

```bash
adb exec-out screencap -p > phone.png
```

### Logging on Android

On Windows the test app logs to the terminal through `pretty_env_logger` (`RUST_LOG` sets the
level). On Android, `android_main` sends the `log` crate's output to Android's log (logcat)
through `android_logger`, at level `info` and above, with the tag `feather` (as the C++ engine's
`feather-Logs-android`).

A panic is logged too, at `error` level, with its message and source location: `android_main`
sets a panic hook, since Android would otherwise show only that the process aborted.

### Validation layers

Debug builds turn on Vulkan's validation layers (`VK_LAYER_KHRONOS_validation`), which check
every Vulkan call and log what is wrong (see `feather::instance` lines in the log). Release
builds never use them.

| Build   | Windows                                              | Android                                   |
|---------|------------------------------------------------------|-------------------------------------------|
| debug   | **required**: from the Vulkan SDK; start-up fails without them (`Validation layer requested but not supported.`) | used if the APK carries them; otherwise a warning, and the app runs without them |
| release | off                                                  | off                                       |

Android devices don't have the validation layers, and the APK doesn't carry them yet, so a debug
build on a device logs at start-up:

```
Validation layer VK_LAYER_KHRONOS_validation not found; running without validation.
```

Problems in Vulkan use are therefore found on Windows: run the same demo there in a debug build
first. (Packing the layers into debug APKs is planned: see the plan's *Later*.)

### Surface and swapchain

At start-up and on each resume, the engine logs what the window's surface offers and what it
chose (`feather::swapchain`). On this Windows machine:

```
Surface: current extent 1280x960, image extent 1x1 to 16384x16384, window 1280x960, current transform IDENTITY, supported transforms IDENTITY, composite alpha OPAQUE, 2 to 8 images
Surface formats: [(R8G8B8A8_UNORM, SRGB_NONLINEAR), (R8G8B8A8_SRGB, SRGB_NONLINEAR), (B8G8R8A8_UNORM, SRGB_NONLINEAR), (B8G8R8A8_SRGB, SRGB_NONLINEAR), (R16G16B16A16_SFLOAT, SRGB_NONLINEAR)]; present modes: [IMMEDIATE, MAILBOX, FIFO, FIFO_RELAXED]
Swapchain: B8G8R8A8_SRGB SRGB_NONLINEAR, 1280x960, MAILBOX, transform IDENTITY, composite alpha OPAQUE
```

These are the first lines to read when the image on a device looks wrong. What the engine
chooses, and what typically differs on Android:

| Choice          | Rule                                                        | Typical on Android |
|-----------------|-------------------------------------------------------------|--------------------|
| Format          | `B8G8R8A8_SRGB`, else `R8G8B8A8_SRGB`, else the first one    | `R8G8B8A8_SRGB` |
| Present mode    | Windows: `MAILBOX`, else `FIFO` (always there); Android: `FIFO` | `FIFO` (vsync) |
| Composite alpha | `OPAQUE`, else `INHERIT`, ...                                | often only `INHERIT` |
| Transform       | `IDENTITY` when supported, else the surface's current one    | `IDENTITY`, with current `ROTATE_90` or `ROTATE_270` on a phone in landscape |
| Extent          | the surface's current extent; the window's size when the display is rotated and the image isn't | the window's (landscape) size |

- **Format.** If the chosen format isn't an `_SRGB` one, colors come out too dark: the shaders
  write linear colors and expect the format to encode them.
- **Present mode.** `MAILBOX` draws as many frames as the GPU can, showing the newest at each
  display refresh; the Galaxy S25 offers it, and the test app ran at 650 frames a second, nearly
  all of them never shown, costing battery and heat. On Android the engine takes `FIFO`: one
  frame per display refresh.
- **Transform.** A phone's natural orientation is portrait, and the test app is landscape, so
  the display is rotated. The engine draws the image unrotated and lets Android's compositor
  rotate it, which costs the compositor a little work (drawing pre-rotated is in the plan's
  *Later*). Android then reports every present as *suboptimal*; the engine knows it chose this
  and doesn't recreate the swapchain for it (otherwise it would, every frame).

### Device features

Some Vulkan features the engine uses are optional on Android GPUs. The engine turns each on only
when the device has it, and logs what it found at start-up:

```
Selected physical device (`Qualcomm(R) Adreno(TM) X1-85 GPU`).
MSAA samples: _4
Device features: DeviceFeatures { sampler_anisotropy: true, max_sampler_anisotropy: 16.0, sample_rate_shading: false }
```

(That is this Windows machine's GPU.) Without a feature, rendering still works, slightly less
smooth:

- `sampler_anisotropy`: textures seen at a slant are blurrier (no anisotropic filtering);
  `max_sampler_anisotropy` caps how much is used (at most 16).
- `sample_rate_shading`: with MSAA, the inside of textured surfaces is less smooth; edges are
  still antialiased. It is off unless the app asks for it (`RenderSettings::sample_shading`,
  from `FeatherApp::render_settings`), as it runs the fragment shader several times per pixel;
  the log then shows `false` even when the device has it.

MSAA uses at most 4 samples per pixel by default (`RenderSettings::max_msaa_samples`), the
largest count up to that which the device supports.

The multisampled color and depth images are only used inside the render pass (never stored), so
they are transient and take lazily allocated memory when the device has it, as phone GPUs do.
It is then never committed, since the GPU keeps them in its tile memory. The log shows it:

```
Transient 2340x1080 image (R8G8B8A8_SRGB, _4): lazily allocated memory, 41119744 bytes.
Transient 2340x1080 image (D32_SFLOAT, _4): lazily allocated memory, 41119744 bytes.
```

On the Galaxy S25 this saves about 80 MB of graphics memory (`adb shell dumpsys meminfo
com.kashikume.feather.testapp`, `GL mtrack`).

## Making an app run on Android


### How the engine runs on Android

The engine uses [winit](https://github.com/rust-windowing/winit) for the window and events on
every platform. On Android, winit runs on top of the
[`android-activity`](https://github.com/rust-mobile/android-activity) crate, with its
**GameActivity** backend: the workspace's `Cargo.toml` turns it on with winit's
`android-game-activity` feature. The feature only matters when building for Android; Windows
builds are unchanged.

`android-activity` contains GameActivity's native code, so the app's Java side has to use the
same GameActivity version. The versions go together:

| winit  | `android-activity` | GameActivity (native, bundled) | Java side                               |
|--------|--------------------|--------------------------------|-----------------------------------------|
| 0.30   | 0.6.1              | 4.4.0                          | `androidx.games:games-activity:4.4.0`   |
| 0.29   | 0.5.2              | 2.0.2                          | `androidx.games:games-activity:2.0.2`   |

The engine uses winit 0.30. When upgrading winit (or `android-activity`), check which
GameActivity the new `android-activity` bundles (`#define GAMEACTIVITY_*_VERSION` in its
`GameActivity.h`, or its changelog) and change the Java side to match.

`feather` re-exports `android-activity`'s `AndroidApp` as `feather::AndroidApp` (Android only):
the handle to the running activity that `android_main` receives. Apps use it from there and don't
need `android-activity` in their own `Cargo.toml`.

The Vulkan surface (what the engine draws into) is made from the window by `feather::surface`.
`vulkanalia`, the Vulkan bindings, makes surfaces for Windows, Linux and macOS windows but not
Android ones, so on Android the engine makes it itself from the window's `ANativeWindow`, with
the `VK_KHR_android_surface` extension.

### The app as a library, and `android_main`

On Windows an app is an executable with a `main`. On Android the system starts a Java activity,
which loads the app as a shared library (`.so`) and calls its entry point, `android_main`, on a
thread of its own. So an app that runs on both is a **library** with the app's code, plus a
small `main.rs` for Windows and an `android_main` for Android. The test app is laid out so:

| File             | What                                                                |
|------------------|---------------------------------------------------------------------|
| `src/lib.rs`     | the library: the demos (`testapp` module), and `android` on Android |
| `src/main.rs`    | Windows: command line, logging, window, `App::run`                  |
| `src/android.rs` | Android: `android_main`                                             |

In its `Cargo.toml`:

```toml
[lib]
name = "feather_testapp"
crate-type = ["rlib", "cdylib"]

[target.'cfg(target_os = "android")'.dependencies]
android_logger = "0.15"
```

- `cdylib` makes the shared library (`libfeather_testapp.so`; on Windows a
  `feather_testapp.dll`, unused); `rlib` lets `main.rs` use the library.
- The library has its own name, not the package's (`testapp`): on Windows, a library and a
  binary of the same name would write clashing `.pdb` files. The Android side loads it by this
  name (`feather_testapp`).
- Log lines from the app's code now name the library (`feather_testapp::testapp::...`), on
  Windows too.

`android_main` (in `src/android.rs`) does what `main` does on Windows, with Android's pieces:

```rust
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(/* level info, tag "feather" */);
    std::panic::set_hook(Box::new(|info| log::error!("{info}")));
    if let Err(e) = run(app) {
        log::error!("{e:#}");
    }
    unsafe { _exit(0) }   // see below
}

fn run(app: AndroidApp) -> Result<()> {
    let event_loop = EventLoop::builder().with_android_app(app.clone()).build()?;
    let window = Window::default_attributes().with_title(demo_window_title(0));
    let assets = Box::new(AndroidAssetsProvider::new(&app));
    App::run(event_loop, window, Box::new(TestApp::with_assets(assets, 0)?))
}
```

- `#[no_mangle]`: `android-activity` calls the function by its plain name.
- The event loop is built **with the `AndroidApp`** (`EventLoopBuilderExtAndroid`), so winit
  gets the activity's events.
- The app passes only the window's attributes: the engine makes the window on the first
  `resumed`, as winit 0.30 requires; on Android the native window (and so the Vulkan surface)
  exists only from then on (see *Lifecycle*).
- Assets come from the APK (`AndroidAssetsProvider`); `TestApp::new`, which reads them from
  next to the executable, exists on Windows only.
- The app starts with demo 0: there is no command line on Android.
- The process ends at the end: Android may call `android_main` again in the same process for a
  new activity, and winit can't make a second event loop in one process. With C's `_exit`, not
  `std::process::exit`: the latter runs C++ static destructors, and GameActivity's
  (`GameTextInput`) then aborts the process on a stale JNI reference (see *Troubleshooting*).

### Leaving the app

Back (the button or the gesture) ends the app, as GuitarPrompter does. It takes the two sides:

1. `MainActivity`'s back callback calls `nativeRequestExit()`, a native method that the Rust
   library exports under its JNI name
   (`Java_com_kashikume_feather_testapp_MainActivity_nativeRequestExit`, in `android.rs`). It
   calls `feather::app::request_exit()`.
2. The callback then finishes the activity (`finish()`).
3. On the Rust side, `App::run`'s loop sees the request and ends: the engine and the app are
   destroyed (`on_destroy`, the Vulkan objects), `android_main` returns and ends the process.

Finishing the activity alone isn't enough: winit 0.30 doesn't end its loop when the activity is
destroyed on Android (it logs a *TODO* and carries on), so `android_main` would never return,
and the process would stay, frozen, until Android killed it; opening the app again in that
process would fail. The JNI name depends on the activity's package and class: an app with
another package exports its own `Java_<package>_<class>_nativeRequestExit`.

Leaving by the *recent apps* screen (swiping the app away) kills the process without any of
this, which is fine: nothing needs saving.

### The Gradle project

`apps\testapp\android` is a standard Android project, written for current versions (checked in
November 2026; GuitarPrompter's project is older and was not copied):

```
apps/testapp/android/
  settings.gradle.kts           repositories, the project's name, its one module (app)
  build.gradle.kts              the plugin, declared for the module
  gradle.properties             memory for Gradle
  gradle/libs.versions.toml     every version, in one place
  gradlew, gradlew.bat, gradle/wrapper/   the Gradle wrapper (downloads Gradle 9.9.1)
  local.properties              where the SDK is (per machine, not committed)
  .gitignore, .gitattributes
  app/
    build.gradle.kts            the app: SDK levels, dependencies, the cargo task
    src/main/AndroidManifest.xml
    src/main/kotlin/com/kashikume/feather/testapp/MainActivity.kt
    src/main/res/values/strings.xml, themes.xml
```

| What                       | Version          | Why                                                     |
|----------------------------|------------------|---------------------------------------------------------|
| Gradle                     | 9.9.1            | latest; runs on Android Studio's JDK 25 (9.1 or newer)  |
| Android Gradle plugin      | 9.5.1            | latest; needs Gradle 9.6.0 or newer; Kotlin built in    |
| `compileSdk` / `targetSdk` | 37 (Android 17)  | latest                                                  |
| `minSdk`                   | 30 (Android 11)  | a recent floor; the engine needs only Vulkan 1.0        |
| NDK (`ndkVersion`)         | 30.0.16248370    | Gradle strips the Rust library's symbols with its tools |
| GameActivity               | 4.4.0            | must match the Rust side (see *How the engine runs on Android*); 4.4.2 is newer but not what `android-activity` 0.6.1 was built with |
| AppCompat                  | 1.8.0            | latest; GameActivity is an `AppCompatActivity`          |
| AndroidX Core              | 1.19.1           | latest; full screen (`WindowInsetsControllerCompat`)    |

The Android Gradle plugin 9 compiles Kotlin itself: there is no Kotlin plugin, and the Kotlin
sources are in `src/main/kotlin`.

**The cargo task.** `app\build.gradle.kts` defines `CargoNdkTask`, registered once per build
variant (`cargoNdkDebug`, `cargoNdkRelease`). It runs, from the workspace's root:

```powershell
cargo ndk -t arm64-v8a -P 30 -o <task output>\jniLibs build -p testapp --lib --release
```

and then copies `target\aarch64-linux-android\release\assets` into `<task output>\assets`.
Through the Android Gradle plugin's variant API (`androidComponents.onVariants`, with
`addGeneratedSourceDirectory`), its two output directories become the APK's native libraries
and assets: Gradle runs the task before packing, and nothing is written into `src\`. The assets
are copied afresh each time, so assets deleted from the sources don't linger in the APK (they do
in `target\...\assets`, which `build.rs` never empties). The task runs on every build; Cargo
decides what to rebuild.

**`AndroidManifest.xml`:**

- `<uses-feature android:name="android.hardware.vulkan.version" android:version="0x400000"
  android:required="true"/>`: the app needs Vulkan 1.0 (Google Play hides it from devices
  without).
- One activity, `.MainActivity`, the launcher's entry (`MAIN` / `LAUNCHER`).
- `android:screenOrientation="sensorLandscape"`: landscape, either way up. (On large screens,
  Android 16 and later ignore orientation locks of apps targeting 36 or more; on phones it
  holds.)
- `android:configChanges="orientation|screenSize|screenLayout|smallestScreenSize|keyboard|keyboardHidden|navigation|density|uiMode"`:
  the activity is never recreated for these changes; it is told and goes on (the window's new
  size reaches the engine as a resize). Recreating it would start a second `android_main` in the
  process, which winit refuses.
- `<meta-data android:name="android.app.lib_name" android:value="feather_testapp"/>`: the
  library GameActivity loads (`libfeather_testapp.so`) and whose `android_main` it calls.

**`MainActivity.kt`** is a `GameActivity` that only sets up the window:

- edge to edge (`enableEdgeToEdge()`), and the status and navigation bars hidden with
  `WindowInsetsControllerCompat` (a swipe from the edge shows them for a moment); hidden again
  whenever the activity gets the focus back, as dialogs and notifications bring them back;
- the screen kept on (`FLAG_KEEP_SCREEN_ON`);
- back, through `onBackPressedDispatcher` (see *Leaving the app*). Apps targeting Android 16 or
  later get back only through these callbacks (predictive back), not as `KEYCODE_BACK` or
  `onBackPressed()`; without a callback, Android would move the app to the background.

**The theme** (`themes.xml`) is `Theme.AppCompat.NoActionBar` (GameActivity needs an AppCompat
theme), with a black window background (before the first frame) and
`windowLayoutInDisplayCutoutMode` `always`, so the image goes under the camera cut-out too.

To make another app's Android project from this one, change: the package and application id
(`namespace`, `applicationId`, the Kotlin file's package and folder), the library's name (in the
manifest's `lib_name`, and `-p testapp` in the cargo task), the JNI name of
`nativeRequestExit` on the Rust side, and the app's name in `strings.xml`.

### Assets

An app reads its assets (shaders, atlases, meshes, scene files) through an `AssetsProvider`, so
the same code works on both platforms:

| Platform | Provider                | Where assets are read from                                 |
|----------|-------------------------|------------------------------------------------------------|
| Windows  | `WindowsAssetsProvider` | files in `assets\` next to the executable                  |
| Android  | `AndroidAssetsProvider` | the APK's `assets/` directory, through Android's `AAssetManager` |

On Windows, `create_assets_provider("assets")` makes the provider. On Android, the app builds it
in `android_main` from the `AndroidApp` it receives, and hands it to the app the same way:

```rust
let assets: Box<dyn AssetsProvider> = Box::new(AndroidAssetsProvider::new(&app));
```

Asset names are the same on both: paths relative to the assets root with `/` separators, e.g.
`"demo_000_viking_room_shader.vert.spv"`. Inside the APK they are read-only, and the APK holds
exactly what `build.rs` put in the `assets\` directory (copied afresh by the cargo task: see
*The Gradle project*).

Meshes are `.fmesh` files made from the OBJs by `build.rs`. For an Android target
(`CARGO_CFG_TARGET_OS` is `android`), `feather_build::build_meshes` writes them uncompressed:
the APK deflates its assets anyway, and the device then inflates each mesh only once. Their load
times appear in logcat: `N meshes built in X ms` per scene file (see
[fmesh-format.md](fmesh-format.md#measurements-and-decisions)).

A missing asset fails with `asset <name> not found in the APK`.

### Touch

On Android, the engine passes touches to the app as the mouse with its left button, as
GuitarPrompter does, so an app written for the mouse (and featherui's buttons and scrollbars)
works with a finger without changes. One finger is followed at a time: the first one down,
until it lifts; others are ignored meanwhile.

| Finger    | `FeatherApp` calls                                             |
|-----------|----------------------------------------------------------------|
| down      | `on_mouse_moved(x, y)`, then `on_mouse_button(Left, true, x, y)` |
| moves     | `on_mouse_moved(x, y)`                                         |
| up        | `on_mouse_button(Left, false, x, y)`, then `on_mouse_left()`   |
| cancelled (the system took the gesture over) | same as up          |

The move before the press lets hover-based UI find what is under the finger; `on_mouse_left`
after the release clears the hover, so a button doesn't stay highlighted. Positions are window
pixels from the top left, as for the mouse.

There is no hover without a press on a touch screen, no other buttons and no wheel. Multi-finger
gestures (pinch, two-finger scroll) are not passed on yet.

On Windows, touches are not mapped: Windows already turns touch-screen touches into mouse
messages, and mapping them too could press a button twice. (The mapping is in
`feather::touch`, with unit tests. That Windows doesn't also deliver touches as winit touch
events alongside its mouse messages, or the other way round, hasn't been tried on a touch
screen.)

## Troubleshooting

### `cargo check --target aarch64-linux-android` fails in `android-activity`'s build script

```
error: failed to run custom build command for `android-activity v0.6.1`
...
  cargo:rerun-if-changed=game-activity-csrc/game-activity/GameActivity.h
  cargo:rerun-if-changed=game-activity-csrc/game-activity/GameActivity.cpp
```

The build script compiles GameActivity's C++ code and finds no compiler for Android. Use
`cargo ndk -t arm64-v8a check` (or `build`) instead: it points the build at the NDK's `clang`.

### `Either "game-activity" or "native-activity" must be enabled as features`

```
error: Either "game-activity" or "native-activity" must be enabled as features
   --> ...\android-activity-<version>\src\lib.rs
error[E0583]: file not found for module `activity_impl`
```

winit is used without an Android backend. The workspace's `Cargo.toml` must have
`winit = { version = "0.30", features = ["android-game-activity"] }`.

### `adb devices` lists nothing, or the install waits forever

```
- waiting for device -
```

The phone isn't connected for debugging: USB debugging is off (it can turn itself off, e.g.
after a system update), or the *Allow USB debugging?* prompt wasn't accepted, or the phone was
locked when it was connected. Check *Developer options → USB debugging* (see *The device*),
reconnect the cable, and unlock the phone. A Gradle install stuck at *waiting for device* can be
stopped with Ctrl+C.

### `Unable to strip the following libraries, packaging them as they are: libfeather_testapp.so`

The Android Gradle plugin strips native libraries with the NDK's `llvm-strip`, and doesn't know
which NDK to use. `ndkVersion` in `app\build.gradle.kts` must name an installed NDK (see
*Setup*). Without stripping the library is 4.3 MB instead of 2.7 MB in the APK; nothing else
changes.

### After back, the app's process stays (frozen) and the app fails to open again

The activity was finished but `android_main` never returned: winit 0.30 doesn't end its event
loop when the activity is destroyed on Android. The activity must ask for it before finishing,
through `nativeRequestExit()` (see *Leaving the app*). With it, the log shows
`Exit requested.` and `The feather test app has ended.` after back.

### `Fatal signal 6 (SIGABRT)` in `android_main` right after `The feather test app has ended.`

```
F libc    : Fatal signal 6 (SIGABRT), code -1 (SI_QUEUE) in tid ... (android_main), pid ... (feather.testapp)
F DEBUG   :       #04 pc ...  /apex/com.android.art/lib64/libart.so (art::JavaVMExt::JniAbort(char const*, char const*)+1752)
F DEBUG   :       #09 pc ...  base.apk (GameTextInput::~GameTextInput()+36)
F DEBUG   :       #11 pc ...  libc.so (__cxa_finalize+220)
F DEBUG   :       #14 pc ...  base.apk (std::process::exit+24)
```

The process was ended with `std::process::exit`, which runs C++ static destructors;
GameActivity's `GameTextInput` destructor then uses a JNI reference that is no longer valid, and
Android aborts. End the process with `_exit` instead (as `android.rs` does): everything that
matters was destroyed already.

### The HUD (or another long-lived scene) disappears after switching to a big scene

Seen on the Galaxy S25 with the test app's HUD after switching to the City demos: the HUD was
gone and came back only after the app went to the background and back.

Each frame's uniform data lives in a buffer that grows when a frame needs more (the log says
`Uniform buffer of swapchain image 0 grown to 256 KB (handle ... -> ...)`). Pipelines cache
their descriptor sets, which point at that buffer, and rewrite them when the buffer is
replaced. The engine used to notice the replacement by the buffer's Vulkan handle, but Android's
driver can give the new buffer the handle of the one just destroyed, and on the S25 it always
did:

```
I feather : feather::app: Uniform buffer of swapchain image 0 grown to 512 KB (handle Buffer(0xb40000770f24b7b0) -> Buffer(0xb40000770f24b7b0)).
```

so the sets kept pointing at freed memory. Scenes that are replaced (the demos) got new pipelines and were fine; the HUD,
drawn throughout, wasn't. The engine now tells buffers apart by an id of its own
(`FrameUniformBuffer::id`). Windows' driver gives new handles, so it never showed there.

In general: don't assume a Vulkan handle identifies an object after it is destroyed; on Android
handles are often addresses, and addresses get reused.
