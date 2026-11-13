# Android: open items

The test app runs on Android (see [../android.md](../android.md)); the plan it followed and its
steps are in [../archive/done-android-plan.md](../archive/done-android-plan.md).

## Not checked yet

- The other landscape side (`sensorLandscape`): needs the phone turned by hand (a forced
  rotation through `adb` changed nothing visible).
- Locking and unlocking the screen (goes through the same `suspended` / `resumed` as
  background and back).
- Following `doc/android.md`'s *Setup* on a clean user account or a second machine.

## Ideas

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
- **Asset enumeration** on Android (`AAssetManager_openDir`; *Asset enumeration* in the missing features note), once the desktop has it.
