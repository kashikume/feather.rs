# Android: plan

First goal: **the test app, with all its demos, running on an Android device**, switched with the
HUD's buttons, and the back button (or back gesture) leaving the app. The Android side follows GuitarPrompter's (`guitarprompter/platform/android`):
a Gradle project with a Kotlin `GameActivity`, landscape, full screen, assets packed into the APK
and read through `AAssetManager`, logs going to logcat, and touches passed to the app as the
left mouse button.

Covers items 21 and 35 of [missing-features-vs-cpp.md](missing-features-vs-cpp.md), and the part
of item 16 that Vulkan needs (losing the window's surface).

## What GuitarPrompter does

- **Gradle project** in `platform/android`: AGP 8.8.0, Kotlin 1.9, `minSdk` 30, `compileSdk` /
  `targetSdk` 33, one `app` module, namespace `com.kashikume.guitarprompter`. The native library
  is built by `externalNativeBuild { cmake }` from the root `CMakeLists.txt`, which makes it a
  `SHARED` library on Android and links `game-activity`, `EGL`, `GLESv3`, `android`, `log`.
  Dependencies: `androidx.games:games-activity:1.2.2`, `appcompat`, `core-ktx`, `material`
  (and `oboe` for audio, not needed here).
- **`AndroidManifest.xml`**: one activity, `.MainActivity`, `screenOrientation="landscape"`,
  `android.app.lib_name` meta-data naming the native library.
- **`MainActivity.kt`**: extends `GameActivity`, loads the library in a `companion object`,
  hides the system UI (immersive sticky) and sets `FLAG_KEEP_SCREEN_ON` in
  `onWindowFocusChanged`. The rest (file import, browser intents) is GuitarPrompter's own.
- **`main.cpp`**: `android_main(android_app*)` creates the app with
  `AssetManager(pApp->activity->assetManager)`, sets `onAppCmd` (forwarding each `APP_CMD_*` to a
  method), sets a motion event filter (pointer and joystick sources only), then loops:
  `ALooper_pollOnce(0, ...)` to process pending events, and while a window exists, handle input
  and render, until `destroyRequested`.
- **`AndroidApp`**: `APP_CMD_INIT_WINDOW` creates the EGL display, surface and GLES 3 context and
  calls `Cache::onSurfaceCreated()` (rebuilds every GPU resource); `APP_CMD_TERM_WINDOW` destroys
  them. Every frame `updateLayout()` queries the surface size and relayouts the view when it
  changed. `handleInput()` swaps the input buffer and maps touches to the mouse:
  `DOWN` / `POINTER_DOWN` to left down, `UP` / `POINTER_UP` / `CANCEL` to left up, `MOVE` to a
  move for every pointer. Keys are only logged.
- **Assets**: the `Makefile`'s `copy_android_assets` copies the built atlases, meshes, shaders
  and configs into `app/src/main/assets`, which Gradle packs into the APK. `AssetManager` reads
  them with `AAssetManager_open` / `AAsset_read`, and lists directories with
  `AAssetManager_openDir`.
- **Logging**: `feather-Logs-android` sends log streams to `__android_log_print` with tag
  `feather`, one priority per level.
- **Images**: `feather-png-android` and friends decode through `jnigraphics`. Not needed in Rust:
  the `png` crate decodes on every platform.

## What differs in Rust

- **Vulkan, not GLES.** Losing the window destroys only the `VkSurfaceKHR` and the swapchain. The
  instance, device, meshes, atlases and pipelines' shaders survive, so there is no equivalent of
  `Cache::onSurfaceCreated` rebuilding everything: only the swapchain and what hangs off it
  (render pass, framebuffers, color and depth images, pipelines) is rebuilt, which
  `recreate_swapchain` already does.
- **winit and `android-activity` replace `native_app_glue`.** winit 0.30 runs on Android through
  `android-activity` 0.6.1, with the `android-game-activity` feature (the engine started on winit
  0.29 / `android-activity` 0.5.2 and was moved to 0.30 for a current GameActivity, below):
  it brings its own copy of the GameActivity C sources and `native_app_glue`, and turns
  `APP_CMD_*` and input into winit events. The app's entry point is
  `#[no_mangle] fn android_main(app: AndroidApp)`, and the event loop is built with
  `EventLoopBuilderExtAndroid::with_android_app`. The C++ command handler maps onto winit events:

  | GuitarPrompter                    | winit                                          |
  |-----------------------------------|------------------------------------------------|
  | `APP_CMD_INIT_WINDOW`             | `ApplicationHandler::resumed`                  |
  | `APP_CMD_TERM_WINDOW`             | `ApplicationHandler::suspended`                |
  | `updateLayout()` size check       | `WindowEvent::Resized`                         |
  | motion events                     | `WindowEvent::Touch { phase, location, id }`   |
  | `destroyRequested`                | `ApplicationHandler::exiting`                  |

- **GameActivity 4.4.0, not 1.2.2.** `android-activity` compiles GameActivity's C sources,
  and the Java side must match exactly. With winit 0.29 (`android-activity` 0.5.2) that was
  2.0.2, from 2023; winit 0.30 (`android-activity` 0.6.1) bundles 4.4.0, so the engine moved to
  winit 0.30 and the app depends on `androidx.games:games-activity:4.4.0`.
- **Gradle builds the Rust library with `cargo ndk`**, not `externalNativeBuild { cmake }`: an
  `Exec` task runs `cargo ndk -t arm64-v8a -o app/src/main/jniLibs build -p testapp --lib`
  before the APK is packed.
- **Assets are built by `build.rs` already.** It copies resources, compiled SPIR-V and the packed
  atlases to `target/<triple>/<profile>/assets/`. Shaders are compiled and fonts rendered on the
  host (build scripts run on the host when cross-compiling), so nothing changes there. Gradle
  takes that directory as an assets source directory instead of a `Makefile` copying files.

## Design for Rust

### Engine (`crates/feather`)

**winit with GameActivity.** The workspace's `winit` gets the `android-game-activity` feature
(it only pulls `android-activity` when building for Android). `feather` re-exports
`winit::platform::android::activity::AndroidApp` on Android, so apps don't need their own
`android-activity` dependency.

**Engine created on `Resumed`, surface lost on `Suspended`.** On Android the window has no
native window before `Resumed`, so no surface can be made, and Vulkan needs the surface to pick a
physical device (present support). `App::run` changes shape: it takes the event loop, the window
and a factory for the `FeatherApp`, and creates the Vulkan objects on the first `Resumed`. winit
sends `Resumed` at start-up on every platform, so Windows takes the same path.

- First `Resumed`: what `App::create` does today.
- `Suspended`: wait for the device to go idle, destroy the swapchain (`destroy_swapchain`) and the
  surface, and stop rendering.
- Later `Resumed`: create the surface again, check the graphics / present queue families still
  present to it, recreate the swapchain, call `on_resized` with the new size.
- `LoopExiting`: `destroy()`, which today runs on `CloseRequested` only; Android never sends
  `CloseRequested`.

The app's scenes and the GPU state of their meshes and atlases stay as they are across
`Suspended` / `Resumed`; pipelines are already recreated on demand after `destroy_swapchain`.

**Surface.** `vulkanalia` 0.25's `window` module handles Win32, X11, Wayland and AppKit, not
Android. A new `surface.rs` gives `required_instance_extensions(window)` and
`create_surface(instance, window)`: on Android `VK_KHR_surface` + `VK_KHR_android_surface`, and
`create_android_surface_khr` with the `ANativeWindow` from `RawWindowHandle::AndroidNdk`;
elsewhere it calls `vk_window`. `vulkanalia::loader::LIBRARY` is already `libvulkan.so` on
Android.

**Validation layers.** Debug builds require `VK_LAYER_KHRONOS_validation` today; devices don't
have it unless the APK carries it. On Android, a missing layer logs a warning and continues
without it. Packing the layer into the APK is in *Later*.

**Device features.** `sampler_anisotropy` and `sample_rate_shading` are required today. Both are
common on Android GPUs but not guaranteed: enable each only when supported, and set the sampler's
`anisotropy_enable` / `max_anisotropy` (clamped to `max_sampler_anisotropy`) from that.

**Swapchain on Android.**

- *Format*: prefer `B8G8R8A8_SRGB`, then `R8G8B8A8_SRGB` (what Android usually offers), then the
  first one. Without the second step the image is drawn linear into a UNORM surface and looks
  too dark.
- *Composite alpha*: `OPAQUE` if supported, else `INHERIT` (many Android drivers support only
  `INHERIT`).
- *Transform*: the activity is landscape and most phones are portrait, so `current_transform` is
  usually `ROTATE_90` or `ROTATE_270`. Passing it as `pre_transform`, as today, would require the
  engine to rotate its own rendering. For the first goal use `IDENTITY` (supported everywhere)
  and let the compositor rotate; the extent comes from the window size when `current_extent`
  doesn't match it. Proper pre-rotation is in *Later*.
- *Suboptimal*: with `IDENTITY` on a rotated display, `vkQueuePresentKHR` returns
  `SUBOPTIMAL_KHR` every frame, and `render` would recreate the swapchain every frame. Recreate on
  `OUT_OF_DATE_KHR` and on resize only; ignore `SUBOPTIMAL_KHR` on Android.
- *Present mode*: `MAILBOX` is rare on Android; the existing `FIFO` fallback is right.
- Log the surface's capabilities (formats, transforms, composite alpha, extents) once at start-up:
  the device-specific values are what to look at first when something is off.

**Touch as the mouse**, as GuitarPrompter does, in `App::run`: the first finger down is followed
(its `id` kept until it lifts); other fingers are ignored.

| `TouchPhase` | calls                                                                 |
|--------------|-----------------------------------------------------------------------|
| `Started`    | `on_mouse_moved(x, y)`, then `on_mouse_button(Left, true, x, y)`      |
| `Moved`      | `on_mouse_moved(x, y)`                                                |
| `Ended`      | `on_mouse_button(Left, false, x, y)`, then `on_mouse_left()`          |
| `Cancelled`  | same as `Ended` (GuitarPrompter treats `CANCEL` as up)                |

The move before the press lets featherui's hover state find the button under the finger;
`on_mouse_left` clears the hover after the finger lifts, so a button doesn't stay highlighted.
Touch positions are physical pixels from the top left, like mouse positions, so featherui needs no
change.

**Assets.** `AndroidAssetsProvider` (`androidassetsprovider.rs`, `cfg(target_os = "android")`)
wraps `ndk::asset::AssetManager` (from `AndroidApp::asset_manager()`), the Rust form of
`AssetManager(AAssetManager*)`: `get_asset(name)` opens `name` from the APK's assets root and
reads it whole. The app builds it from its `AndroidApp` and passes it in, like GuitarPrompter
passes the asset manager to `Application`; `create_assets_provider` stays the Windows (desktop)
entry point. `windowsassetsprovider` stays `cfg(windows)`.

**Logging.** `android_logger` with tag `feather`, as `feather-Logs-android` does, set up by the
app's `android_main`, plus a panic hook that logs the panic message and location at `error`
level (panics otherwise vanish from logcat).

### Test app (`apps/testapp`)

**A library as well as a binary.** Android loads a shared library, not an executable. The
package gets `src/lib.rs` (`[lib] name = "feather_testapp"`, `crate-type = ["rlib", "cdylib"]`)
holding the `testapp` module and, on Android, `android_main`:

```rust
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    // android_logger + panic hook
    // event loop with_android_app(app.clone()), window
    // App::run(..., || TestApp::with_assets(Box::new(AndroidAssetsProvider::new(&app)), 0))
    // std::process::exit(0) once the loop ends
}
```

`main.rs` keeps the command line and `pretty_env_logger`, using the library. The library name
differs from the binary's (`testapp`) so that Windows doesn't build a `testapp.dll` and a
`testapp.exe` with clashing `.pdb` files.

`std::process::exit(0)` after the loop: Android may start `android_main` again in the same
process, and winit refuses to create a second event loop. The manifest's `configChanges`
also keeps the activity from being recreated on rotation and keyboard changes.

The HUD needs nothing new: its buttons switch demos with the finger, and `set_scale(height /
768)` sizes it for the screen.

### Gradle project (`apps/testapp/android`)

Mirrors `guitarprompter/platform/android`, without what only GuitarPrompter needs (oboe,
material, file import):

Not GuitarPrompter's setup, which is outdated (AGP 8.8, Gradle 8.10, Kotlin plugin 1.9, SDK 33,
GameActivity 1.2.2, `systemUiVisibility` flags, `onKeyDown` for back): current versions and
APIs throughout, checked in November 2026.

```
apps/testapp/android/
  settings.gradle.kts           plugin and dependency repositories, rootProject, include(":app")
  build.gradle.kts              plugins declared, not applied
  gradle.properties             JVM memory for the Gradle daemon
  gradle/libs.versions.toml     version catalog: every version in one place
  gradlew, gradlew.bat, gradle/wrapper/   Gradle 9.9.1
  local.properties              sdk.dir (not committed)
  app/
    build.gradle.kts            the app, and the cargo task
    src/main/AndroidManifest.xml
    src/main/kotlin/com/kashikume/feather/testapp/MainActivity.kt
    src/main/res/values/{strings,themes}.xml
```

| What                    | Version          | Why                                                          |
|-------------------------|------------------|--------------------------------------------------------------|
| Gradle                  | 9.9.1 (latest)   | runs on Android Studio's JDK 25 (from 9.1); AGP 9.5 needs ≥ 9.6.0 |
| Android Gradle plugin   | 9.5.1 (latest)   | Kotlin built in (no `org.jetbrains.kotlin.android` plugin)   |
| `compileSdk` / `targetSdk` | 37 (latest, installed as `android-37.2`) | AGP 9.5 supports up to 37 |
| `minSdk`                | 30 (Android 11)  | a recent floor (the engine needs only Vulkan 1.0); also `cargo ndk -P 30` |
| GameActivity            | 4.4.0            | must match `android-activity` 0.6.1's bundled native code (latest is 4.4.2, but not what the native side was built from) |
| AppCompat               | 1.8.0 (latest)   | GameActivity is an `AppCompatActivity`; the theme           |
| AndroidX Core           | 1.19.1 (latest)  | `WindowCompat` / `WindowInsetsControllerCompat` for full screen |

- **`app/build.gradle.kts`**: namespace and application id `com.kashikume.feather.testapp`;
  `ndk { abiFilters += "arm64-v8a" }`; no `externalNativeBuild`.
- **The cargo task** (`CargoNdkTask`, in `app/build.gradle.kts`): runs
  `cargo ndk -t arm64-v8a -P 30 -o <out>/jniLibs build -p testapp --lib [--release]` from the
  workspace root, then copies `target/aarch64-linux-android/<profile>/assets` into
  `<out>/assets` (a fresh copy: no stale assets in the APK). Wired with the **Variant API**
  (`androidComponents.onVariants`: `sources.jniLibs` and `sources.assets`
  `.addGeneratedSourceDirectory(task, ...)`), so Gradle knows the task produces the APK's
  libraries and assets, runs it before packing, and nothing is written into `src/`. Both
  variants build Rust with `--release` by default (unoptimized Rust is too slow for most demos
  on a phone); `-Pfeather.rustProfile=debug` switches to a Rust debug build.
- **`AndroidManifest.xml`**: one activity, `.MainActivity`, `exported`,
  `screenOrientation="sensorLandscape"` (either landscape side), `configChanges` covering
  orientation, size, layout, keyboard, density and UI mode (the activity is never recreated: see
  `android_main`'s `process::exit`), `android.app.lib_name` = `feather_testapp`, and
  `<uses-feature android:name="android.hardware.vulkan.version" android:version="0x400000"
  android:required="true"/>` (Vulkan 1.0, what the engine asks for). (On large screens, Android 16+ ignores orientation
  locks for apps targeting 36+; on a phone it holds.)
- **`MainActivity.kt`**: `class MainActivity : GameActivity()`; loads `feather_testapp` in a
  `companion object`; in `onCreate`, `FLAG_KEEP_SCREEN_ON`, edge to edge and the system bars
  hidden with `WindowInsetsControllerCompat` (`BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE`), again in
  `onWindowFocusChanged` (they come back after dialogs); the back callback (below).
- **Back leaves the app**, through `OnBackPressedDispatcher`: since targeting Android 16 (API
  36), back (button or gesture) no longer arrives as `KEYCODE_BACK` or `onBackPressed()`, only
  through the dispatcher's callbacks (predictive back). The callback calls `finish()`: the
  activity is destroyed, winit's loop ends, the engine is destroyed in `exiting`, and
  `android_main` ends the process. (Android's default for a root activity is now to move the
  task to the background instead of finishing it; the plan wants back to leave.)
- **Theme**: `Theme.AppCompat.NoActionBar` with a black window background and
  `windowLayoutInDisplayCutoutMode` `always`, so the image covers the camera cut-out area too.
- No launcher icon at first (Android's default).

Building and running:

```powershell
rustup target add aarch64-linux-android
cargo install cargo-ndk
cd apps/testapp/android
./gradlew installDebug
adb shell am start -n com.kashikume.feather.testapp/.MainActivity
adb logcat -s feather RustStdoutStderr AndroidRuntime
```

The machine setup (SDK, NDK, JDK, Rust target, `cargo-ndk`) is in
[doc/android.md](../android.md#setup).

## Documentation

The Android setup and build are documented in `doc/android.md`, written as the work goes: every
step below adds its part to the doc in the same commit, with commands that were run on this
machine and the output that shows they worked. Nothing goes in the doc unverified; what was
expected but not yet tried stays in this plan.

The doc is the first piece of a future tutorial for building a feather app for Windows and
Android (see *Later*), so it is written for someone who has not set up Android development
before, and keeps two kinds of content apart:

- **Setup** (once per machine): what to install, which version, where it ends up, which
  environment variables, and a command to check each one.
- **Per app** (once per app): what an app needs to run on Android (a library target with
  `android_main`, a Gradle project, assets), with the test app as the example, and where its
  names (library, package, application id) appear, so another app can copy it.

Sections of `doc/android.md`:

1. **Overview**: how the pieces fit: Gradle → `cargo ndk` → `libfeather_testapp.so` +
   `assets/` → APK → device; a diagram of the lifecycle (`Resumed` / `Suspended` / back).
2. **Setup**, one subsection per tool, each with *install*, *configure*, *check*:
   - Android SDK (platform 33, build tools, platform tools / `adb`) and `ANDROID_HOME`;
   - NDK (version, `ANDROID_NDK_HOME`, the x86-64 toolchain under emulation on ARM64 Windows);
   - JDK (Android Studio's `jbr`, JDK 25 here) and `JAVA_HOME`;
   - Rust target `aarch64-linux-android` and `cargo-ndk`;
   - Vulkan SDK (`glslc` on the host; no layers on the device);
   - the device: developer options, USB debugging, `adb devices`, checking Vulkan support.
3. **Building and running**: `gradlew installDebug` / `installRelease`, the `rustProfile`
   property, starting the app with `adb`, building the Rust library alone with `cargo ndk`,
   `cargo check` for Android.
4. **Logs and debugging**: `adb logcat` filters, the panic hook, the surface capabilities logged
   at start-up, validation layers (absent, and how to add them later).
5. **Making an app run on Android**: the library target and `android_main`, the assets provider,
   the Gradle project's files and the names to change in them, the manifest's settings and why
   (landscape, `configChanges`, `lib_name`, Vulkan feature), `MainActivity` (immersive mode,
   back).
6. **Troubleshooting**: each problem met during the steps, with its symptom (the log line or
   error message) and the fix.

The README links to it from *Requirements* and *Building*, and keeps only the short version.

## Steps

Every step ends with its part of `doc/android.md` (in brackets below), verified on this machine.

1. **Toolchain.** *Done.* Install the `aarch64-linux-android` target and `cargo-ndk`; check the
   JDK and the NDK are found. No code changes. [Doc: *Overview* skeleton, the whole *Setup* section
   except the device, done tool by tool as each is installed and checked.]
2. **The engine compiles for Android.** *Done.* `winit` with `android-game-activity`; `feather`
   re-exports `AndroidApp`. Nothing else needed a `cfg`: the Windows-only pieces
   (`windowsassetsprovider`, `create_assets_provider`) were already `cfg(windows)`.
   `cargo ndk -t arm64-v8a check -p feather -p feather-ui` passes (a plain
   `cargo check --target` can't build GameActivity's C++). Desktop tests unchanged. [Doc: `cargo check` for Android in *Building and running*.]
3. **`AndroidAssetsProvider`.** *Done, untested until step 9.* Keeps a clone of `AndroidApp`
   and opens each asset through `asset_manager()`, so `feather` needs no `ndk` dependency.
   Tested on the device in step 9 (it needs an APK); on the desktop only that it compiles. [Doc: the assets provider in *Making an app run on Android*.]
4. **Surface and instance on Android**: *Done, untested on a device until step 9.* `surface.rs`,
   optional validation layer on Android (`AppData::validation`, decided at start-up, replacing
   three `VALIDATION_ENABLED` constants), optional `sampler_anisotropy` and
   `sample_rate_shading` (`DeviceFeatures` in `AppData`, used by the device, the texture sampler
   and `PipelineTarget::sample_shading`). Desktop: demo 16 runs with validation on and no
   errors. [Doc: validation layers in *Logs and debugging*.]
5. **Lifecycle**: *Done.* `App::run(window, event_loop, app)` (the app boxed, not a factory:
   creating it doesn't need Vulkan, only `on_create` does, which `App::create` calls), engine
   created on the first `Resumed`, surface and swapchain dropped on `Suspended` and rebuilt on
   `Resumed` (both do nothing when repeated, as winit asks), `destroy()` on `LoopExiting`.
   Errors creating, resuming or drawing end the loop and are returned instead of panicking.
   Instead of a test (it needs a Vulkan device), debug builds suspend and resume on **F9**:
   checked on Windows with demos 14 to 19, F9 four times between demo switches, no validation
   errors, clean exit on closing the window. [Doc: the
   lifecycle diagram in *Overview*; what an app may assume across `Suspended` / `Resumed`.]
6. **Swapchain on Android**: *Done, untested on a device until step 9.* Format, composite alpha,
   `IDENTITY` transform and extent, each a small function with unit tests (8). `SUBOPTIMAL_KHR`
   is ignored not by platform but when the swapchain was made with a transform other than the
   surface's current one (`Swapchain::suboptimal_expected`), so a real change still recreates
   it everywhere. The surface's capabilities and the choices are logged at start-up and on
   resume, not on resizes. Windows unchanged (`B8G8R8A8_SRGB`, `MAILBOX`, `IDENTITY`,
   `OPAQUE`). [Doc: reading
   the surface capabilities log in *Logs and debugging*.]
7. **Touch as the mouse** in `App::run`. *Done, untested on a device until step 9.* The
   mapping is `feather::touch::TouchAsMouse` (6 unit tests), used by `App::run` on Android only:
   Windows sends mouse messages for touch screens itself. Tests (no window needed if the mapping is a small
   function from `(phase, id, x, y)` and the followed finger to `FeatherApp` calls): first finger
   followed, second ignored, `Cancelled` releases, `on_mouse_left` after `Ended`. [Doc: touch in
   *Making an app run on Android*.]
8. **Test app library**: *Done, untested on a device until step 9.* `lib.rs`, `android.rs`
   with `android_main`, `android_logger` and the panic hook, `main.rs` using the library;
   `TestApp::new` (assets next to the executable) is Windows only. The `.so` needs only system
   libraries (no `libc++_shared.so`) and exports `android_main`. `cargo run` and `cargo test --workspace` unchanged;
   `cargo ndk -t arm64-v8a build -p testapp --lib` produces `libfeather_testapp.so`. [Doc: the
   library target and `android_main`; building the Rust library alone; the panic hook.]
8b. **winit 0.30.** *Done.* For a current GameActivity (4.4.0 instead of 2.0.2): winit 0.30.13,
   `android-activity` 0.6.1. `App::run(event_loop, window_attributes, app)`: `Runner` is an
   `ApplicationHandler`, and the window is made on the first `resumed` (winit 0.30 makes windows
   only from a running loop). Windows checked as in step 5 (demos 20 to 22, F9 twice, clean
   exit).
9. **Gradle project** *Done.* In `apps/testapp/android`, as above (current versions, not
   GuitarPrompter's). Found on the device: winit 0.30 doesn't end its loop when the activity is
   destroyed, so back calls `feather::app::request_exit()` through a JNI method before
   finishing; `std::process::exit` aborts in GameActivity's C++ static destructors, so the
   process ends with `_exit`; `MAILBOX` ran demo 1 at 650 fps, so Android uses `FIFO`; the
   Android Gradle plugin needs `ndkVersion` to strip the library. Checked on a Galaxy S25:
   demos 0 to 2 draw, touch switches demos, home and return keep the demo, back exits cleanly
   twice in a row. First milestone: the APK installs,
   starts, logs to logcat and shows demo 0; back (button and gesture) returns to the launcher,
   logcat shows the engine destroyed, and launching again starts cleanly. [Doc: *Setup*'s
   device subsection; *Building and running* complete; the Gradle project, manifest and
   `MainActivity` in *Making an app run on Android*; `adb logcat` filters.]
10. **Every demo on the device.** Found so far: the HUD disappears from the City demos on and
    stays away until the app is suspended and resumed. Cause:
    the frame's uniform buffer grows (to 512 KB entering demo 15 on the phone, 1 MB on 16), and the pipelines' cached
    descriptor sets were rewritten only when the buffer's *handle* changed; Android drivers can
    give the new buffer the old one's handle, so the HUD's sets (its pipelines outlive demo
    switches) kept pointing at the freed buffer. Fixed by comparing `FrameUniformBuffer::id`, a
    number never reused; growth is logged with both handles. (Windows gives new handles, so it
    never showed there.) Confirmed on the S25: every grown buffer got its predecessor's
    handle (`0xb40000770f24b7b0 -> 0xb40000770f24b7b0`); with the fix the HUD stays on demos
    15, 16 and after them.
    Then: Go through demos 0–22 with the HUD buttons, then put the app in
    the background and back, and lock and unlock the screen, on each of a few demos. What
    breaks is fixed or written down in this plan's *Device notes* (to add): likely candidates are
    shader features a mobile GPU lacks, the size of the shadow maps and atlases, MSAA sample
    counts, and the frame rate of the heavier demos (15, 16). [Doc: *Troubleshooting*, from what
    was met here and in the earlier steps; the device the demos were checked on.]
11. **Notes**: README (Android in *Requirements* and *Building* as a short version linking to
    `doc/android.md`, touch in *Features*); in the missing features note, items 21 and 35 move to
    covered features, and item 16 notes that on Vulkan only the surface and swapchain are lost.
    Read `doc/android.md` through from the top as a newcomer would, and follow its *Setup* on
    a clean user account or a second machine if one is available.

## Later

- **Tutorial: a feather app for Windows and Android**, in `doc/tutorial/`: from an empty
  directory to an app with a scene, a HUD and input, running on both. Its setup chapter is
  `doc/android.md`'s *Setup* plus the Windows setup (moved from the README's *Requirements* into
  a `doc/` page of its own, in the same install / configure / check form). Its Android chapter
  builds on *Making an app run on Android*. The Gradle project may by then become a template
  (names as Gradle properties) so a new app copies it without editing.
- **Pre-rotation**: pass `current_transform` as `pre_transform` and rotate the projection in the
  engine, avoiding the compositor's rotation pass (Android's recommended Vulkan practice).
- **Validation layers in debug APKs**: put `libVkLayer_khronos_validation.so` from the
  Vulkan-ValidationLayers releases into `jniLibs/arm64-v8a`.
- **`x86_64` for the emulator**: second ABI; the emulator's Vulkan support varies, so a real
  device stays the reference.
- **Lazily allocated MSAA and depth attachments** (`TRANSIENT_ATTACHMENT` +
  `LAZILY_ALLOCATED`): no memory traffic for them on tiled mobile GPUs.
- **Touch beyond one finger**: drag to scroll in featherui (see the featherui plan's *Later*),
  pinch.
- **Density-aware HUD scale**: size the HUD from the screen's DPI (`AConfiguration_getDensity`)
  instead of its height, so buttons are finger-sized on small screens.
- **Release signing**, launcher icon, app name per demo.
- **Texture handles in the descriptor set cache**: `ShaderPipeline::descriptor_sets` is keyed
  by `(image view, sampler)` handles. A driver may reuse a destroyed image view's handle for a
  new one (as happened with buffers, step 10), so a scene that removes an atlas and uploads
  another could draw with sets pointing at the freed view. Key by atlas ids instead, or drop a
  pipeline's cached sets when an atlas it used is released.
- **Stale assets**: `build.rs` copies into `assets/` but never removes, so assets deleted or
  renamed in the sources stay there (19 such files in `target/debug/assets` today) and would
  be packed into the APK. Clear the directory first, or copy to a fresh one.
- **Asset enumeration** on Android (`AAssetManager_openDir`, item 20), once the desktop has it.
