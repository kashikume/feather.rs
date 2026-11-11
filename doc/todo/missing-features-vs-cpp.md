# Features missing compared to the C++ Feather engine

Reference: the C++ engine used by GuitarPrompter,
`guitarprompter/contrib/feather` (`src/feather`, `src/featherui`,
`tools`). Each item names the C++ source it comes from and what feather.rs has today.

Features feather.rs already covers (not listed below): scene graph with transforms and visibility,
materials with uniforms, uniform inheritance down the hierarchy, PNG atlases with metadata,
textures as atlas regions, cuboid / plane / OBJ mesh builders, text in the XY and XZ planes
(one mesh or one node per glyph), transparency sorting back to front, directional shadow maps,
named lookup of resources, the atlas creator and font creator tools. feather.rs also has things
C++ never had (Vulkan, SPIR-V reflection, sphere builder, skeletons and animation clips, JSON
scene files, perspective/orthographic cameras, several scenes per frame).

## Animation

Done (see [animation-plan.md](animation-plan.md)): `animator.rs`, `animation.rs`, `easing.rs`,
scene animations (`Scene::add_animation`) and demo 21.

1. **Animator** (`feather-Animator`): a container that owns running animations, updates them with
   the frame's time delta and drops the finished ones.
2. **SlideAnimation** (`feather-Animation`): a time-limited animation calling a step function with
   progress 0..1 and a finish callback; can be finished early.
3. **PerpetualAnimation** (`feather-Animation`): an endless animation called with the current and
   delta time, with a way to reset its current time.
4. **Easing functions** (`feather-Utils.h`): linear, quadratic, cubic, quartic, quintic, sine,
   circular, exponential, elastic, back and bounce, each in in/out/in-out variants, plus
   `easeInOut3`.

## Scene graph / nodes

5. **Rendering priority** (`Node::setRenderingPriority`, optionally passed to children): an
   explicit draw order that wins over the depth sort, for both opaque and transparent nodes.
6. **Ordered children** (`Node::getChildAt`, `getChildrenCount`): children are kept in insertion
   order and can be accessed by index; in Rust they are a `HashSet`.
7. **Reattach a node to another parent** (`Node::reattach`): Rust can only disconnect a node
   (`Scene::disconnect_node`), not attach it under a new parent.
8. **Set material on a whole subtree** (`Node::set_material(material, passToChildren)`).
9. **Per-node textures** (`Node::set_texture`, `get_texture`): a node can override or add texture
   slots on top of its material's.
10. **"Receives shadow" flag** (`Node::Flags::DisplaysShadow`): a per-node switch for whether the
    node shows shadows cast on it; Rust only has `casts_shadow`.
11. **Effective visibility query** (`Node::isEffectivelyVisible`): whether a node is visible
    taking its ancestors into account.
12. **Transform convenience API** (`Node::setPos`, `setScale`, `x`/`y`/`z` and
    `global_x`/`global_y`/`global_z` properties): set or read position and scale without building
    a whole matrix.
13. **Shader override per node** (`Node::shader`): a node can use a different shader than its
    material's.

## Rendering

14. **Camera sort modes** (`CameraType` in `feather-RenderingState`): sort transparent (and opaque)
    nodes along X, Y or Z, ascending or descending, for 2D and 3D cameras, as alternatives to
    sorting by camera distance.
15. **Opaque nodes sorted front to back** (`RenderingBatcher::sortOpaqueNodes`): Rust groups
    opaque draws by shader only.
16. **Configurable clear mode and clear color** (`RenderingState::setClearMode`,
    `setClearColor`): Rust always clears to black and only chooses whether to clear depth between
    scenes.
17. **Rendering statistics** (`RenderingState::m_stats.nodesRendered`,
    `RenderingBatcher::getNodesCount`): number of nodes drawn in a frame.
18. **Built-in shader generator** (`ShaderProgram::Create(flags)`): GLSL generated at runtime from
    flags `SpecLight`, `Shadow`, `Transparency`, `Glow`, `SingleColor`, so simple materials need no
    hand-written shaders. Rust needs every shader written and compiled to SPIR-V by the app.
19. **Multiple textures per material** (`Material::get_textures`, `texture` lines in the material
    file): several sampler slots, each with its own atlas region (`texUmin0`, ...). Rust has one
    texture per material and binds every sampler to its atlas.
20. **Graphics context loss recovery** (`Cache::onSurfaceCreated`, `reset()` on atlases, meshes,
    shaders, materials and roots): rebuild every GPU resource after the surface is destroyed
    (needed on Android).

## Resources and assets

21. **Global resource cache shared between scenes** (`feather-Cache`): atlases, meshes, shaders
    and materials stored once by name and reused by every root/scene, with garbage collection of
    unused entries and `getStats()`. In Rust each `Scene` owns its own resources.
22. **Material description file** (`feather-Material`, `assets/materials.mdb`): text format with
    `@material` sections, `shader`, `texture` (atlas / single), `uniform` (float, int, vec3, vec4,
    RGB, RGBA), `font`, `version` and `define` / `%name` macros. Rust has JSON scene files instead;
    loading `.mdb` would let GuitarPrompter's assets be reused unchanged.
23. **Font atlas attached to a material** (`Material::font_atlas`, `TextBuilder(material)`): build
    text directly from a material that names its font.
24. **Asset enumeration** (`AssetManager::enumerate_files`): list the files of an asset directory.
25. **Android asset provider** (`AssetManager(AAssetManager*)`): read assets from the APK.
26. **Typed loading errors** (`feather-Exceptions.h`: `FileNotFound`, `FileReadError`,
    `IncorrectFileFormat`, `UnsupportedFileVersion`, `MaterialNotFound`): callers can tell failure
    kinds apart; Rust returns opaque `anyhow` errors.

## Images and textures

27. **JPEG loading** (`feather-jpeg`): decode JPEG images into atlases/textures.
28. **`.tex` texture file format** (`Atlas::CreateFromTexFile`, `Texture::CreateFromTexFile`): the
    engine's own pre-processed texture format.
29. **Mutable atlases** (`Atlas::setTexel`, `m_isMutable`, `buildTexBuffer`): change pixels at
    runtime and re-upload the texture.
30. **Texture from a raw buffer** (`Texture::CreateFromBuffer(w, h, format, buffer)`), including
    **RGB (3-byte) pixel format** (`PixelFormat::RGB`); Rust atlases are RGBA only.
31. **Texture size in pixels** (`Texture::getWidth`, `getHeight`, `getSize`).

## Meshes

32. **Ribbon mesh builder** (`feather-MeshBuilderRibbon`): a strip built from pairs of points with
    UVs, with normals generated automatically.
33. **Custom mesh builder** (`feather-MeshBuilderCustom`): add points (position, normal, UV) and
    faces (polygons, optionally inverted) one by one.
34. **Mesh merger** (`feather-MeshMerger`): combine two meshes into one.
35. **Plane builder variants** (`feather-MeshBuilderPlane`): planes in the XZ plane, centered unit
    planes (`unitCXY`, `unitCXZ`), planes with custom UV ranges, and the **9-patch plane**
    (`build9pInXY`) for scalable UI panels.
36. **Binary `.mesh` file loader** (`feather-MeshLoaderMeshFile`) and the generic
    **`MeshLoader`** that picks the loader by file extension.
37. **Coordinate system preview** (`CreateCoordinateSystemPreview`): a helper that adds X/Y/Z axes
    gizmo nodes under a parent, for debugging.

## Input and application

38. **Mouse input** (needed by `featherui::ui_hud::mouse_move` / `mouse_button_down` /
    `mouse_button_up`): `FeatherApp` only receives key presses.
39. **Thread-safe scene objects** (`ThreadsafeObject`, `FEATHER_THREADSAFE`): optional locking so
    the scene can be modified from a thread other than the render thread, and a debug check that
    catches access from the wrong thread.
40. **Profiler integration** (Tracy `ZoneScoped` in `types.h`).
41. **Android platform support** (`feather-Logs-android`, `feather-png-android`,
    `feather-image-android`, `feather-jpeg-android`): logging, image decoding and app lifecycle on
    Android.

## UI library (`featherui`)

42. **UI object base** (`featherui::object`): rectangle, width/height, left/right/top/bottom,
    visibility (`automatic`, `gone`, `invisible`, `visible`), `update(anchor, rect)` layout pass,
    mouse event hooks.
43. **Layouts** (`featherui::layout`, `layout_nswe`): a container with north / south / east /
    west / center slots.
44. **Controls with dragging** (`featherui::control`): mouse down/up/drag handling.
45. **Scrollbars** (`featherui::scrollbar`, `vertical_scrollbar`, `horisontal_scrollbar`): min /
    max / value / page, minimum knob size, knob and track meshes and materials (normal and
    highlighted), value-changed observers, auto-hide when not needed.
46. **Spacer control** (`featherui::distance`).
47. **UI HUD** (`featherui::ui_hud`): owns a UI root with its own batcher and rendering state,
    lays out the root layout in a rectangle, renders it and routes mouse events (including drag
    capture) to the hit object.

## Tools

48. **ObjMeshConverter** (`tools/ObjMeshConverter`): converts OBJ files (with axis order and
    fixed-point factor options) into the binary `.mesh` format, one file per object.
