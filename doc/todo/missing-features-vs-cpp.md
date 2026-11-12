# Features missing compared to the C++ Feather engine

Reference: the C++ engine used by GuitarPrompter,
`guitarprompter/contrib/feather` (`src/feather`, `src/featherui`,
`tools`). Each item names the C++ source it comes from and what feather.rs has today.

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

## Scene graph / nodes

1. **Rendering priority** (`Node::setRenderingPriority`, optionally passed to children): an
   explicit draw order that wins over the depth sort, for both opaque and transparent nodes.
2. **Ordered children** (`Node::getChildAt`, `getChildrenCount`): children are kept in insertion
   order and can be accessed by index; in Rust they are a `HashSet`.
3. **Reattach a node to another parent** (`Node::reattach`): Rust can only disconnect a node
   (`Scene::disconnect_node`), not attach it under a new parent.
4. **Set material on a whole subtree** (`Node::set_material(material, passToChildren)`).
5. **Per-node textures** (`Node::set_texture`, `get_texture`): a node can override or add texture
   slots on top of its material's.
6. **Effective visibility query** (`Node::isEffectivelyVisible`): whether a node is visible
   taking its ancestors into account.
7. **Transform convenience API** (`Node::setPos`, `setScale`, `x`/`y`/`z` and
   `global_x`/`global_y`/`global_z` properties): set or read position and scale without building
   a whole matrix.
8. **Shader override per node** (`Node::shader`): a node can use a different shader than its
   material's.

## Rendering

9. **Camera sort modes** (`CameraType` in `feather-RenderingState`): sort transparent (and opaque)
    nodes along X, Y or Z, ascending or descending, for 2D and 3D cameras, as alternatives to
    sorting by camera distance.
10. **Opaque nodes sorted front to back** (`RenderingBatcher::sortOpaqueNodes`): Rust groups
    opaque draws by shader only.
11. **Configurable clear mode and clear color** (`RenderingState::setClearMode`,
    `setClearColor`): Rust always clears to black and only chooses whether to clear depth between
    scenes.
12. **Rendering statistics** (`RenderingState::m_stats.nodesRendered`,
    `RenderingBatcher::getNodesCount`): number of nodes drawn in a frame.
13. **Built-in shader generator** (`ShaderProgram::Create(flags)`): GLSL generated at runtime from
    flags `SpecLight`, `Shadow`, `Transparency`, `Glow`, `SingleColor`, so simple materials need no
    hand-written shaders. Rust needs every shader written and compiled to SPIR-V by the app.
14. **Multiple textures per material** (`Material::get_textures`, `texture` lines in the material
    file): several sampler slots, each with its own atlas region (`texUmin0`, ...). Rust has one
    texture per material and binds every sampler to its atlas.

## Resources and assets

15. **Global resource cache shared between scenes** (`feather-Cache`): atlases, meshes, shaders
    and materials stored once by name and reused by every root/scene, with garbage collection of
    unused entries and `getStats()`. In Rust each `Scene` owns its own resources.
16. **Material description file** (`feather-Material`, `assets/materials.mdb`): text format with
    `@material` sections, `shader`, `texture` (atlas / single), `uniform` (float, int, vec3, vec4,
    RGB, RGBA), `font`, `version` and `define` / `%name` macros. Rust has JSON scene files instead;
    loading `.mdb` would let GuitarPrompter's assets be reused unchanged.
17. **Font atlas attached to a material** (`Material::font_atlas`, `TextBuilder(material)`): build
    text directly from a material that names its font.
18. **Asset enumeration** (`AssetManager::enumerate_files`): list the files of an asset directory.
19. **Typed loading errors** (`feather-Exceptions.h`: `FileNotFound`, `FileReadError`,
    `IncorrectFileFormat`, `UnsupportedFileVersion`, `MaterialNotFound`): callers can tell failure
    kinds apart; Rust returns opaque `anyhow` errors.

## Images and textures

20. **JPEG loading** (`feather-jpeg`): decode JPEG images into atlases/textures.
21. **Mutable atlases** (`Atlas::setTexel`, `m_isMutable`, `buildTexBuffer`): change pixels at
    runtime and re-upload the texture.
22. **Texture from a raw buffer** (`Texture::CreateFromBuffer(w, h, format, buffer)`), including
    **RGB (3-byte) pixel format** (`PixelFormat::RGB`); Rust atlases are RGBA only.
23. **Texture size in pixels** (`Texture::getWidth`, `getHeight`, `getSize`).

## Meshes

24. **Ribbon mesh builder** (`feather-MeshBuilderRibbon`): a strip built from pairs of points with
    UVs, with normals generated automatically.
25. **Custom mesh builder** (`feather-MeshBuilderCustom`): add points (position, normal, UV) and
    faces (polygons, optionally inverted) one by one.
26. **Mesh merger** (`feather-MeshMerger`): combine two meshes into one.
27. **Plane builder variants** (`feather-MeshBuilderPlane`): planes in the XZ plane, centered unit
    planes (`unitCXY`, `unitCXZ`), planes with custom UV ranges, and the **9-patch plane**
    (`build9pInXY`) for scalable UI panels.
28. **Coordinate system preview** (`CreateCoordinateSystemPreview`): a helper that adds X/Y/Z axes
    gizmo nodes under a parent, for debugging.

## Input and application

29. **Thread-safe scene objects** (`ThreadsafeObject`, `FEATHER_THREADSAFE`): optional locking so
    the scene can be modified from a thread other than the render thread, and a debug check that
    catches access from the wrong thread.
30. **Profiler integration** (Tracy `ZoneScoped` in `types.h`).

## Animation ideas (not in C++ either)

- Replace-by-name: `add_animation` with a name already running cancels the old one, so two slides
  don't fight over one node (GuitarPrompter avoids this by hand with separate animators).
- Generation counters in handles, so stale handles are detected (would apply to every `ObjDB`).
- Animations declared in scene files (target node, uniform or transform, duration, easing).
- Reaching app state from scene animations: their callbacks only get `&mut Scene`, so anything
  else must be moved into the closure (like the `Text`s of demos 11 and 12) or handled by a
  standalone `Animator` with its own context. Demo 11's episode spawning stays in `update`
  because it needs the demo's random generator and counter. If this comes up often, consider
  storing `Text`s in the scene, or letting scenes hold user data.

## UI ideas (not in C++ either)

- **9-patch track and knob** (item 27): rounded knobs and buttons that don't stretch their
  corners.
- **Clipping**: a scissor rectangle per scene or per node, so a scrolled list is cut at the
  view's edges. Today widgets must stay in their rectangle themselves (demo 22's list hides the
  lines and letters outside its view).
- **Buttons that fade in**: the test app's side buttons could show only while the pointer is
  near their edge, with a scene animation on their `alpha`.
- **A vertical stack layout**, so panels like the test app's `InfoPanel` can be built from
  labels instead of being a widget of their own.
- **Widgets in scene files**: a `"ui"` section describing the layout, like cameras and nodes.
- **Keyboard focus** and **touch** (drag to scroll, as GuitarPrompter's `LibraryView` does by
  hand).
- **Removing a `Label`** leaves its one-mesh text's mesh in the scene (widgets are dropped
  without the scene); a widget teardown hook with the scene would fix it.

## Camera ideas (not in C++ either)

- The same scene drawn through two cameras (split screen, minimap, mirror): a camera override in
  `FeatherApp`, or the renderer drawing each of the scene's enabled cameras into its own
  viewport. Today a scene is drawn once, through its active camera.
- A camera attached to a node, following its global transform (e.g. riding a car in demo 15).
- Transitions between cameras: blend view and projection over time instead of cutting.
- `Projection::Custom(Mat4)` for projections the app computes itself.
- Only the active camera gets the image size each frame, so an inactive orthographic camera's
  `get_visible_size` reports the size from when it was last active.

## Won't do / deprecated

C++ features feather.rs won't port.

- **"Receives shadow" flag** (`Node::Flags::DisplaysShadow`): declared in C++ but never read;
  its generated shader's `iExpectShadow` uniform was hard-coded to 1. In feather.rs a shader can
  declare its own uniform (e.g. `bool receivesShadow`, default in the material) and skip the
  shadow map when it is false; set on a node, it is inherited by the whole subtree.
- **`.tex` texture file format** (`Atlas::CreateFromTexFile`, `Texture::CreateFromTexFile`): the
  C++ engine's own pre-processed texture format.
