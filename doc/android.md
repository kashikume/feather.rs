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

window closed / activity finished ──▶ exiting ──▶ engine and app destroyed
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

*Building the APK and running it on a device: not written yet (plan step 9).*

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

## Logs and debugging

*Reading logs from the device: not written yet (plan step 9).*

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
| Present mode    | `MAILBOX`, else `FIFO` (always there)                        | `FIFO` (vsync) |
| Composite alpha | `OPAQUE`, else `INHERIT`, ...                                | often only `INHERIT` |
| Transform       | `IDENTITY` when supported, else the surface's current one    | `IDENTITY`, with current `ROTATE_90` or `ROTATE_270` on a phone in landscape |
| Extent          | the surface's current extent; the window's size when the display is rotated and the image isn't | the window's (landscape) size |

- **Format.** If the chosen format isn't an `_SRGB` one, colors come out too dark: the shaders
  write linear colors and expect the format to encode them.
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
Device features: DeviceFeatures { sampler_anisotropy: true, max_sampler_anisotropy: 16.0, sample_rate_shading: true }
```

(That is this Windows machine's GPU.) Without a feature, rendering still works, slightly less
smooth:

- `sampler_anisotropy`: textures seen at a slant are blurrier (no anisotropic filtering);
  `max_sampler_anisotropy` caps how much is used (at most 16).
- `sample_rate_shading`: with MSAA, the inside of textured surfaces is less smooth; edges are
  still antialiased.

## Making an app run on Android

*The Gradle project, the manifest and the activity: not written yet (plan step 9).*

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
    std::process::exit(0);
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
- `std::process::exit(0)` at the end: Android may call `android_main` again in the same process
  for a new activity, and winit can't make a second event loop in one process.

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
exactly what `build.rs` put in the `assets\` directory (how Gradle packs it: plan step 9).

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
