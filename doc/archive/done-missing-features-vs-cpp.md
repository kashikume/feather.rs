# Features covered compared to the C++ Feather engine

What feather.rs has of the C++ engine (`guitarprompter/contrib/feather`), and more; the missing
features are in [../todo/missing-features-vs-cpp.md](../todo/missing-features-vs-cpp.md).

Features feather.rs already covers (not listed below): scene graph with transforms and visibility,
materials with uniforms, uniform inheritance down the hierarchy, PNG atlases with metadata,
textures as atlas regions, cuboid / plane / OBJ mesh builders, text in the XY and XZ planes
(one mesh or one node per glyph), transparency sorting back to front, directional shadow maps,
named lookup of resources, animations (animator, slide and perpetual animations, easing
functions), mouse input, the `featherui` UI library (`feather-ui`: hud, NSWE layout, spacer,
scrollbars; plus labels and buttons, which C++ didn't have), Android (assets read from the APK,
logging to logcat, the app's lifecycle: with Vulkan only the window's surface and swapchain are
lost when the app goes to the background, not every GPU resource as with the C++ engine's
OpenGL context; touch as the mouse; see [../android.md](../android.md)), binary meshes (`.fmesh`
files, made from OBJs by the mesh converter, which replaces `ObjMeshConverter`, loaded with the
loader picked by the file's extension; see [../fmesh-format.md](../fmesh-format.md)), the atlas
creator and font creator tools.
feather.rs also has things C++ never had (Vulkan, SPIR-V reflection, sphere builder, skeletons
and animation clips, JSON scene files, cameras as scene objects (perspective or orthographic,
several per scene with an active one, defined in scene files), several scenes per frame).

## Items done

Items removed from the missing features note as they were done, as they read then (their
numbers dropped), restored from git history.

### Animations (see [done-animation-plan.md](done-animation-plan.md))

Removed in 592daed (2026-11-11).

- **Animator** (`feather-Animator`): a container that owns running animations, updates them with
  the frame's time delta and drops the finished ones.
- **SlideAnimation** (`feather-Animation`): a time-limited animation calling a step function with
  progress 0..1 and a finish callback; can be finished early.
- **PerpetualAnimation** (`feather-Animation`): an endless animation called with the current and
  delta time, with a way to reset its current time.
- **Easing functions** (`feather-Utils.h`): linear, quadratic, cubic, quartic, quintic, sine,
  circular, exponential, elastic, back and bounce, each in in/out/in-out variants, plus
  `easeInOut3`.

### Mouse input and the UI library (see [done-featherui-plan.md](done-featherui-plan.md))

Removed in d414c4f (2026-11-11).

- **Mouse input** (needed by `featherui::ui_hud::mouse_move` / `mouse_button_down` /
  `mouse_button_up`): `FeatherApp` only receives key presses.
- **UI object base** (`featherui::object`): rectangle, width/height, left/right/top/bottom,
  visibility (`automatic`, `gone`, `invisible`, `visible`), `update(anchor, rect)` layout pass,
  mouse event hooks.
- **Layouts** (`featherui::layout`, `layout_nswe`): a container with north / south / east /
  west / center slots.
- **Controls with dragging** (`featherui::control`): mouse down/up/drag handling.
- **Scrollbars** (`featherui::scrollbar`, `vertical_scrollbar`, `horisontal_scrollbar`): min /
  max / value / page, minimum knob size, knob and track meshes and materials (normal and
  highlighted), value-changed observers, auto-hide when not needed.
- **Spacer control** (`featherui::distance`).
- **UI HUD** (`featherui::ui_hud`): owns a UI root with its own batcher and rendering state,
  lays out the root layout in a rectangle, renders it and routes mouse events (including drag
  capture) to the hit object.

### Android (see [done-android-plan.md](done-android-plan.md))

Removed in 0ea1d46 (2026-11-11).

- **Graphics context loss recovery** (`Cache::onSurfaceCreated`, `reset()` on atlases, meshes,
  shaders, materials and roots): rebuild every GPU resource after the surface is destroyed
  (needed on Android).
- **Android asset provider** (`AssetManager(AAssetManager*)`): read assets from the APK.
- **Android platform support** (`feather-Logs-android`, `feather-png-android`,
  `feather-image-android`, `feather-jpeg-android`): logging, image decoding and app lifecycle on
  Android.

### Binary meshes (see [done-fmesh-plan.md](done-fmesh-plan.md))

Removed in 760ae4a (2026-11-12).

- **Binary `.mesh` file loader** (`feather-MeshLoaderMeshFile`) and the generic
  **`MeshLoader`** that picks the loader by file extension.
- **ObjMeshConverter** (`tools/ObjMeshConverter`): converts OBJ files (with axis order and
  fixed-point factor options) into the binary `.mesh` format, one file per object.
