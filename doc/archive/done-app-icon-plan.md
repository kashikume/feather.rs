# Test app icon (a feather): plan (done)

Implemented on 2026-11-21; see *Done* at the end. The plan is kept as written.

Goal: the test app gets **a feather icon** on Windows (the `.exe` in Explorer, the window's title
bar and the taskbar) and on Android (the launcher and the recent apps list).

## What GuitarPrompter does

- **Windows**: `res/guitar_prompter.ico` in a resource script (`res/guitarprompter.rc`),
  compiled into the `.exe` by MSVC through CMake. The window class uses it, so the title bar and
  taskbar show it too.
- **Android**: an adaptive icon (`res/mipmap-anydpi/ic_launcher.xml`: a background colour and
  a foreground image), the foreground as PNGs in every density (`mipmap-mdpi` ... `xxxhdpi`),
  plus round and legacy variants, made by Android Studio's Image Asset tool;
  `android:icon="@mipmap/ic_launcher"` in the manifest.
- The C++ feather engine has nothing for icons; the app does it.

## What the test app has

- Windows: no resource, so Explorer shows the default executable icon; the window is made with
  `Window::default_attributes()` and no icon, so the title bar and taskbar show Windows' default.
- Android: no `android:icon` in `AndroidManifest.xml`, so the launcher shows the default
  (green robot) icon.

## Proposal

### One source for every icon

`tools/scripts/make_feather_icon.py` (Python, standard library only, like the other scripts)
describes the icon once as shapes and writes every file from it:

- **The design** (chosen from previews): a white feather (a curved quill, a vane wider on the
  right, its tip leaning to the upper right, three splits in the vane), the right half of the vane
  a pale blue-grey (`#DCE8EE`), the quill light blue-grey (`#8FB3C6`), on a rounded square with
  a **sky gradient**, bright blue at the top (`#3A8DDE`) to deeper blue at the bottom
  (`#1B4F9C`). Readable at 16 × 16.
- `apps/testapp/icon/feather.ico`: 16, 24, 32, 48, 64 and 256 pixels (PNG-compressed entries),
  drawn by the script with antialiasing (supersampled).
- `apps/testapp/icon/feather_256.png`: the same at 256, for the window icon at run time.
- Android: `res/drawable/ic_launcher_foreground.xml`, the feather as a **vector drawable**
  (paths, so no PNG per density), `res/values/ic_launcher_background.xml` (the gradient's colours), and
  `res/mipmap-anydpi/ic_launcher.xml` (the adaptive icon). The launcher cuts adaptive icons to
  its own shape, so the background is the gradient as a vector drawable
  (`res/drawable/ic_launcher_background.xml`, filling the whole square), not the rounded
  square. The app needs API 30, so adaptive
  icons are all it needs; no legacy PNGs.

The generated files are committed (the build doesn't need Python), as with the other scripts.

### Windows

- **The `.exe` icon**: `build.rs` compiles a one-line resource script (`1 ICON "icon/feather.ico"`)
  into the executable with the **`embed-resource`** crate, only when the target is Windows
  (`CARGO_CFG_TARGET_OS`), so Android builds don't change. It uses the Windows SDK's `rc.exe`,
  which the MSVC toolchain already needs.
- **The window icon**: the title bar and taskbar may not pick the `.exe`'s icon up by themselves
  (to check with winit; if they do, this part is dropped). `main.rs` sets it
  with `with_window_icon`, from the PNG built into the program (`include_bytes!`), decoded by the
  engine's PNG reader. A small engine helper, `feather::app::window_icon_from_png(bytes)`, so
  other apps (and the tutorial's) can do the same in one line.

### Android

`android:icon="@mipmap/ic_launcher"` in the manifest. Gradle compiles the resources; nothing
else changes.

### Docs

- README (Building / Project layout): the icon files and the script.
- `doc/android.md`: where the launcher icon comes from, how to change it.

## Steps

1. The script and the generated files; look at them at 16, 32 and 256 pixels.
2. `window_icon_from_png` in the engine (with a test); `main.rs` sets the window icon.
3. `build.rs`: `embed-resource` on Windows targets; check Explorer and the taskbar.
4. Android resources and the manifest; check the launcher on the S25.
5. Docs.

## Decisions

1. **Design**: A (white feather) on the sky gradient (from previews of four designs, then six
   backgrounds).
2. **`embed-resource`**: yes; the icon in Explorer is a must.
3. **Tutorial**: unchanged.

## Done

All steps (2026-11-21), as planned:

- `tools/scripts/make_feather_icon.py` writes `apps/testapp/icon/feather.ico` (16–256 pixels,
  PNG entries), `feather_256.png` and the Android `ic_launcher.xml`,
  `ic_launcher_foreground.xml` and `ic_launcher_background.xml`. Colours are blended in linear
  light; the quill is drawn thicker below 48 pixels.
- `feather::app::window_icon_from_png` (with a test); `main.rs` sets the window icon from
  `feather_256.png`. Checked: without it winit's window shows Windows' default icon, not the
  `.exe`'s, so it stays.
- `build.rs` compiles `icon/testapp.rc` into `testapp.exe` only (not the library) with
  `embed-resource` 3, on Windows targets. Checked: the `.exe`'s icon (as Explorer extracts it)
  and the title bar show the feather.
- The manifest's `android:icon` / `android:roundIcon`; README (requirements: `rc.exe`, layout)
  and `doc/android.md` (*The launcher icon*).

Checked on the Galaxy S25: the app drawer shows the feather on the sky gradient, cut to the
launcher's rounded square, as "Feather test ...".
