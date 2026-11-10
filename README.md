# feather.rs

A lightweight 3D engine written in Rust on top of the Vulkan API.

Targets Windows and Android, with other platforms possible later.

> **Status: early development.** The project is being turned from a Vulkan tutorial-style example
> into the basic parts of an engine. APIs, file layout and module names change often.

## Features

What works today:

- Vulkan renderer (via [`vulkanalia`](https://github.com/KyleMayes/vulkanalia)) with swapchain recreation on resize
- Windowing and event loop via [`winit`](https://github.com/rust-windowing/winit)
- Scene graph made of nodes with transforms, visibility, meshes and materials; every visible node
  with a mesh and a material is drawn
- Scene objects (meshes, nodes, atlases, shaders, materials) stored in the scene and referenced
  by handle
- Mesh builders: Wavefront `.obj` loading (via `tobj`), procedural cuboids, subdivided planes and
  balls (UV spheres)
- Atlases loaded from PNG (any color type, converted to 8-bit RGBA) with optional `.atlas`
  metadata naming their textures, or made from pixels generated in code (`Atlas::from_rgba`);
  textures are plain values taken from an atlas
- Shaders (SPIR-V) and materials (a shader, a texture and uniform values); each shader's uniform
  blocks, push constants and samplers are read from its SPIR-V, and each shader gets its own
  pipeline
- Uniforms on materials and nodes, inherited down the scene graph
- Assets loaded through a platform-specific `AssetsProvider`
- Scenes described in JSON files, with objects referring to each other by name
- Perspective camera, and an orthographic (flat) camera for 2D scenes
- Several scenes drawn together, one over the other, each through its own camera (e.g. a 2D
  overlay over a 3D scene)
- Atlases uploaded with mipmaps; a material's texture passes its UV rectangle to the shader
  (`umin`, `umax`, `vmin`, `vmax` uniforms)
- Atlases built from `.atlist` image lists by the atlas creator during the build, and font
  atlases rendered from Windows fonts described in `.font` files
- Transparent nodes, alpha-blended; draws sorted: opaque nodes by shader, then transparent nodes
  back to front
- Text from font atlases made by the font creator: laid out (alignment including justified,
  several lines, word wrapping, in the XY or XZ plane) into one mesh per text or a node per character, updated when the text changes
  (`TextBuilder`, `Text`)
- Nodes and meshes removable while the scene is shown (`Scene::remove_node`,
  `Scene::remove_mesh`); mesh GPU buffers are released with their last mesh
- Skeletal animation (rigid): bones are scene nodes (`Skeleton`), moved by looping keyframe
  clips loaded from JSON (`AnimationClip`)
- Shadows: a directional light per scene draws the shadow casters into a shadow map before each
  frame, and shaders sample it (`ShadowLight`, `Node::set_casts_shadow`)
- MSAA
- Vulkan validation layers turned on automatically in debug builds

> The renderer draws every visible node that has a mesh and a material, with the material's
> shader and texture, and the node's uniforms. An app can draw several scenes, one over the
> other, each through its own camera. See
> [doc/todo](doc/todo/node-uniforms-rendering.md) for the plan and known limitations.

## Requirements

- **Rust** (stable, edition 2021): <https://rustup.rs>
- **Vulkan SDK**: <https://vulkan.lunarg.com>. You need it for:
  - `glslc`, which the build uses to compile the GLSL shaders to SPIR-V
  - `VK_LAYER_KHRONOS_validation`, which debug builds **require**. Without it, start-up fails with
    `Validation layer requested but not supported.`
- A GPU and driver that support Vulkan

## Building

```powershell
cargo run              # debug build, validation layers on
cargo run --release    # release build, validation layers off
cargo run -- 1         # start with demo 1
cargo test --workspace # unit tests of every crate (plain `cargo test` tests only the test app)
cargo run -p atlas-creator -- --help   # run a tool
```

The repository is a Cargo workspace. `cargo run` and `cargo build` at the root work on the test
app (`default-members`); use `-p <package>` or `--workspace` for the others.

The development app (`apps/testapp`) opens a 1024×768 window and shows one demo at a time. The
only command line argument is the number of the demo to start with (default 0). **Left** and
**right** arrows switch to the previous and next demo; after the last demo comes the first.

| Demo | Name        | What it shows                                                        |
| ---- | ----------- | -------------------------------------------------------------------- |
| 0    | Viking room | A model with a texture, loaded from a [scene file](#scene-files)     |
| 1    | Cuboid      | A textured cuboid from its scene file: a node spun in code holds a node scaled in the file |
| 2    | Atlas       | 16 still plates in a 4×4 grid, each showing one texture of an atlas built from 16 images |
| 3    | Waving atlas | The same plates waving in the wind, bent by the vertex shader       |
| 4    | Lit cuboid  | The tumbling cuboid of demo 1 with diffuse and specular lighting, from the direction in the `light` uniform set on the scene's root node |
| 5    | Globe       | A world map on a ball, spinning around its tilted axis while drifting along an oval path, with a cratered moon going around it; both lit by a strong light from the side: a dark night side, a highlight on water only and a glowing atmosphere on the globe |
| 6    | Bouncing balls | Balls dropping onto a floor plate, bouncing off its edge and out of the screen, in a loop, with semi-transparent shadows; no textures, every object has its own `color` uniform |
| 7    | Vases       | 12 vases of 3 shapes (generated OBJ files with thick walls) in different sizes and colours on a turning floor plate; some opaque, some see-through glass, all from the scene file |
| 8    | Walker      | A textured person walking in a circle: body parts hang from bones, moved by a walk-cycle animation clip |
| 9    | Spider      | A textured spider walking in a circle with the same bone system: 8 legs of 4 bones each, in the alternating tetrapod gait |
| 10   | Text        | Text in two fonts (Segoe UI and Segoe UI Bold, made with the font creator): a title changing colour, lines aligned left, centred and right, Polish, German and Spanish letters, a clock updated ten times a second, a line whose letters (a node each) bob in a wave in their own colours, and text lying on a turning floor |
| 11   | Crawl       | An endless crawl: made-up episodes (number, title, justified paragraphs) sliding away over a twinkling star field and fading out towards the horizon; each episode is added when there is room and removed once it is gone |
| 12   | Overlay     | Two scenes together: demo 0's spinning room behind, and in front a 2D overlay seen by an orthographic camera, with see-through panels, text, the room's texture as an image and the frame rate |
| 13   | Simple shadow | A human figure (a generated mesh) standing on a floor, casting its shadow from a fixed light; nothing moves, the shader takes one shadow-map sample |
| 14   | Shadows     | A sun going round the sky over a ring of stone pillars, a spinning golden cube on a pedestal and bouncing balls; their soft-edged shadows turn and stretch with it |
| 15   | City        | A busy city the camera glides through: 5 x 5 blocks of buildings in four styles, streets with lanes, crosswalks and traffic lights, trees, lamps and a park; over 100 cars stopping at red lights and keeping their distance, over 100 people walking; everything textured (textures painted in code, no image files) and casting shadows from the afternoon sun, with sky reflections on glass and fog into a sky dome; a 2D overlay shows the frame rate |
| 16   | City (assets) | The same city as demo 15, but every model is an OBJ and every texture is a PNG packed into an atlas at build time (`tools/generate_demo_016_city_assets.py`); denser geometry (recessed windows, richer cars, people and trees) |

Each demo is in its own file, `apps/testapp/src/testapp/demo_NNN_<name>.rs`, and its resources
are named `demo_NNN_<name>_<kind>` (`scene`, `atlas`, `mesh`, `shader`, ...). Demos are listed in
`DEMOS` in `apps/testapp/src/testapp/testapp.rs`.

Everything the app loads comes from the `assets/` directory next to the executable, so it can be
started from any directory.

### Shaders

The test app's `build.rs` compiles every shader in `apps/testapp/shaders/` (`.vert`, `.frag`,
`.comp`, `.geom`, `.tesc`, `.tese`) with `glslc` during `cargo build`, and recompiles whenever
anything there changes. Shaders are compiled without `-O`, also in release builds: optimization
strips the debug names the engine reads uniform names from. A compiled shader keeps its path
relative to `shaders/` and gets `.spv` added, so `shaders/demo_000_viking_room_shader.vert`
becomes `demo_000_viking_room_shader.vert.spv`. It is written to:

- the `assets/` directory next to the executable, where applications load it through the assets
  provider as `"demo_000_viking_room_shader.vert.spv"` (see [Assets](#assets)),
- `$OUT_DIR/shaders/`, for embedding with
  `include_bytes!(concat!(env!("OUT_DIR"), "/shaders/demo_000_viking_room_shader.vert.spv"))`.

The engine's own `build.rs` compiles its built-in shaders, from `crates/feather/shaders/` (the
shadow pass's), and the shaders its unit tests use, from `crates/feather/testdata/shaders/`. Both build scripts use the helpers in `crates/feather-build`,
which look for `glslc` in this order: the `GLSLC` environment variable, `$VULKAN_SDK/Bin/glslc`,
then `glslc` on `PATH`.

### Atlases

The test app's `build.rs` also packs the images listed in every `apps/testapp/atlases/<name>.atlist`
into an atlas, with the atlas creator (`tools/atlas-creator`, used as a library through
`feather_build::build_atlases`). An `.atlist` file lists PNG files, one per line, relative to the
list. The result is `<name>.png` (the atlas) and `<name>.atlas` (its metadata: one
`image-name x y width height` line per image, in pixels), copied to `assets/`. Each atlas gets the
smallest size from 64×32, 64×64, 128×64, ... that its images fit in, with a 1-pixel frame
around every image. The generated atlases are not in the repository, only the images and lists:
demo 2's are `apps/testapp/atlases/demo_002_atlas_atlas.atlist` and
`apps/testapp/atlases/demo_002_atlas/plate_01.png` ... `plate_16.png` (demo 3 has its own copy
under `demo_003_waving_atlas`).

### Fonts

Font atlases are made entirely during the build: an `apps/testapp/atlases/<name>.font` file
describes one, and `build_atlases` renders the font's characters with the font creator
(`tools/font-creator`, used as a library) into `$OUT_DIR/atlases/<name>/`, lists them in
`$OUT_DIR/atlases/<name>.atlist` and packs them like any other atlas into `<name>.png` and
`<name>.atlas`. No character images are in the repository. Demo 10's regular font:

```
# Segoe UI, 64 pixels high, with the default characters.
font = Segoe UI
height = 64
```

`font` is a font installed in Windows, by its full name (`Arial Bold`), its file name in the
Windows font directory (`arialbd`) or a path; the build fails if it isn't found. Every image is
`height` pixels high (the font's ascent to descent, with the baseline at the same row in all of
them) and as wide as the character's advance, so images put side by side make the text. Images
are named by the character's code point (`65.png` for `A`), which becomes the texture name in
the `.atlas` file. The default characters are printable ASCII (with the space), Polish, German
and Spanish letters, typographic quotes and apostrophe, ellipsis, the non-breaking space and a
few symbols (`€ ° • § ·`); `chars = ...` replaces them and `add-chars = ...` adds to them.
Characters the font has no glyph for are skipped.

The font creator is also a command line tool, to look at the images or list the fonts:

```powershell
cargo run -p font-creator -- --list    # installed fonts
cargo run -p font-creator -- -f "Segoe UI Bold" -h 32 -o font -a font.atlist
```

## Assets

The test app's `build.rs` copies everything in `apps/testapp/resources/` to an `assets/`
directory next to the executable (e.g. `target/debug/assets/`), together with the compiled
shaders and the generated atlases. It copies again whenever anything in `resources/` changes.

At runtime assets are read through the `AssetsProvider` trait, which has one implementation per
platform:

| Platform | Implementation          | Reads assets from                        |
| -------- | ----------------------- | ---------------------------------------- |
| Windows  | `WindowsAssetsProvider` | a directory relative to the executable   |
| Android  | not implemented yet     | will read from the APK's assets          |

`create_assets_provider` returns the provider for the current platform. Asset names are relative to
the assets root and use `/`, e.g. `"textures/wall.png"`.

```rust
let assets = create_assets_provider("assets")?;

let bytes = assets.get_asset("demo_000_viking_room_shader.vert.spv")?;
let mesh = MeshBuilderObjFile::new(&*assets, "demo_000_viking_room_mesh.obj")?
    .build(&mut scene, Some("Room".to_string()))?;
let atlas = scene.create_atlas_from_png(&*assets, "demo_000_viking_room_atlas.png", None, None)?;
let plates = scene.create_atlas_from_png(
    &*assets,
    "demo_002_atlas_atlas.png",
    Some("demo_002_atlas_atlas.atlas"),
    Some("Plates".to_string()),
)?;
```

Atlases and meshes can also be built from bytes already in memory (`Atlas::new_from_png_bytes`,
`MeshBuilderObjFile::from_bytes`).

## Logging

Logging uses [`pretty_env_logger`](https://docs.rs/pretty_env_logger) and is controlled by `RUST_LOG`.
Validation layer messages also go to the log.

**PowerShell**

```powershell
$env:RUST_LOG="debug"   # or: error, warn, info, trace
cargo run
```

**Bash**

```bash
RUST_LOG=debug cargo run
```

`trace` is very verbose: it logs every frame callback.

## VS Code

`.vscode/` contains ready-made configurations. Recommended extensions are listed in
`.vscode/extensions.json`.

| Launch configuration          | What it does                                          |
| ----------------------------- | ----------------------------------------------------- |
| `Run feather-rs`              | `cargo run` with `RUST_LOG=debug`, no debugger        |
| `Run feather-rs (release)`    | `cargo run --release` with `RUST_LOG=info`            |
| `Debug feather-rs`            | builds, then attaches the MSVC debugger (`cppvsdbg`)  |
| `Debug feather-rs (release)`  | the same for the release build                        |

Tasks: `cargo build`, `cargo build --release`, `cargo run`, `cargo run --release`, `cargo fmt`.

## Project layout

```
Cargo.toml           workspace: members and shared dependency versions
crates/
  feather/           the engine (library crate `feather`)
    build.rs         compiles the engine's shaders in shaders/ and, for the unit tests, those in
                     testdata/shaders/
    shaders/         the engine's built-in shaders (the shadow pass)
    testdata/        small files used only by the unit tests
    src/             engine sources (lib.rs lists the modules):
    app.rs           Vulkan setup, main loop, frame rendering
    featherapp.rs    FeatherApp trait implemented by applications
    scene.rs         scene: owns all objects and the node graph
    scenefile.rs     loading a scene from a JSON file
    objdb.rs         handle-based object storage used by the scene
    node.rs          scene graph node (mesh, material, uniforms)
    mesh*.rs         meshes and mesh builders (.obj file, cuboid, plane, sphere)
    atlas.rs, texture.rs
                     images loaded from PNG with their .atlas metadata, and textures cut from them
    text.rs          text laid out with a font atlas into meshes, and nodes showing it
    gpuimage.rs      atlases uploaded to the GPU, and the texture sampler
    shader.rs, material.rs
                     SPIR-V shaders and materials
    shaderlayout.rs  uniform blocks, push constants and samplers read from a shader's SPIR-V
    uniform.rs, uniforms.rs
                     uniform values and named uniform lists
    uniformdata.rs   packing uniform values into the bytes a shader reads
    assetsprovider.rs, windowsassetsprovider.rs
                     platform-independent asset access and its Windows implementation
    camera.rs, perspectivecamera.rs, orthographiccamera.rs
    shadow.rs, shadowpass.rs
                     shadow lights, and the shadow pass drawing casters into shadow maps
    instance.rs, physicaldevice.rs, logicaldevice.rs, swapchain.rs, pipeline.rs, ...
                     Vulkan object wrappers
  feather-build/     build script helpers: compiling shaders with glslc, creating atlases,
                     copying assets
apps/
  testapp/           development application (binary `testapp`)
    src/main.rs      entry point: creates the window and starts the engine with TestApp
    src/testapp/     one file per demo, testapp.rs switches them
    shaders/         GLSL sources, compiled by build.rs
    atlases/         .atlist image lists and their images, and .font font descriptions, made
                     into atlases by build.rs
    resources/       scene files, models and textures, copied to assets/ by build.rs
    build.rs         compiles shaders, builds atlases and copies assets next to the executable
tools/
  atlas-creator/     packs PNG images into an atlas: command line tool and library
  font-creator/      renders a Windows font's characters to PNG images and an .atlist: command
                     line tool and library
doc/                 scene file format and plans
```

To add a tool, create `tools/<name>/` with its own `Cargo.toml` (use `edition.workspace = true`
and `<dependency>.workspace = true` for dependencies listed in the root `Cargo.toml`). The
workspace picks it up automatically; run it with `cargo run -p <name>`. Tools should not depend
on the `feather` crate unless they need the engine: it requires the Vulkan SDK to build.

## Writing an application

An application implements the `FeatherApp` trait and gives the engine the scenes and cameras to
render:

```rust
pub trait FeatherApp {
    fn on_create(&mut self) -> Result<()>;
    fn on_render(&mut self) -> Result<()>;
    fn on_update(&mut self, time: f32) -> Result<()>;
    fn on_destroy(&mut self);

    fn get_num_scenes_to_render(&self) -> usize;
    fn get_scene_to_render(&mut self, scene_index: usize) -> &mut Scene;
    fn get_camera_to_render_scene(&mut self, scene_index: usize) -> &mut dyn Camera;

    /// Optional: called when a key is pressed.
    fn on_key_pressed(&mut self, key: KeyCode) -> Result<()> { Ok(()) }
}
```

The app may return a different scene at any time. The engine notices (each `Scene` has a unique
`get_id()`), waits for the GPU and releases everything it uploaded for the previous scene, then
uploads the new one. Scenes hold no GPU objects themselves, so the app can simply drop them.

Each demo of the test app loads its scene from a [scene file](#scene-files) and finds the node it
animates by name, e.g. `apps/testapp/src/testapp/demo_000_viking_room.rs`:

```rust
let scene = Scene::load(assets, "demo_000_viking_room_scene.json")?;
let room_node = scene.find_node("Room").unwrap();
```

Scenes can also be built in code, with `create_root_node`, `create_node`, `MeshBuilderCuboid`,
`MeshBuilderPlane`, `MeshBuilderSphere`, `MeshBuilderObjFile`, `create_shader`, `create_atlas_from_png`,
`create_material`, `material_set_texture` and the other `Scene` methods. Demos 2 and 3
(`demo_002_atlas.rs`, `demo_003_waving_atlas.rs`) load their atlas, mesh and shader from a scene
file and create a material and a node for every texture of the atlas in code.

`main.rs` passes the app to the engine:

```rust
let mut app = unsafe { App::create(&window, Box::new(TestApp::new(demo)?))? };
app.run(&window, event_loop)?;
```

### Drawing order and transparency

A node marked transparent (`Node::set_transparent(true)`, or `"transparent": true` in a scene
file) is blended over what is behind it, using the alpha its shader outputs. Each frame the
engine sorts the draws (`scene::sort_for_rendering`):

1. opaque nodes, grouped by shader, so the pipeline changes as rarely as possible;
2. transparent nodes, from the farthest to the nearest (by the distance of each node's origin
   along the camera's view direction), so each one is blended over everything behind it.

Transparent nodes are drawn with a second pipeline of their shader: blending on, and the depth
test on but depth writes off, so they are hidden by opaque objects in front of them but don't hide
each other. The bouncing balls demo uses this for its shadows.

### Shadows

A scene casts shadows when it has a `ShadowLight` (`Scene::set_shadow_light`, or `shadow_light`
in a scene file): a light's view-projection matrix, for a directional light like the sun made by
`ShadowLight::directional(direction, center, radius)`, and the size of its shadow map. Before
each frame, the engine draws every node that casts a shadow (`Node::set_casts_shadow`, or
`"casts_shadow": true`) from the light into the scene's shadow map, a depth image, with a
built-in depth-only shader and a depth bias. This is the shadow mapping of the C++ feather
(`CastShadow` nodes, `shadowPV`, `ShadowGenerateShaderProgram`), with a real depth image instead
of depth packed into RGBA.

Shaders receive shadows by naming two values the engine fills, like the matrices:

```rust
let names = ShaderUniformNames::matrices("model", "view", "proj")
    .with_shadow("lightViewProj", "shadowMap"); // a mat4 uniform, a sampler
```

```glsl
fragLightSpace = camera.lightViewProj * world;   // vertex shader
...
vec3 p = fragLightSpace.xyz / fragLightSpace.w;  // fragment shader
float lit = p.z - bias <= texture(shadowMap, p.xy * 0.5 + 0.5).r ? 1.0 : 0.0;
```

The map is cleared to depth 1 and sampled with nearest filtering and a white border, so places
outside it are lit; in a scene without a light, the sampler gets a map that is lit everywhere.
For a large scene, the light can follow the camera: `ShadowLight::directional_snapped` moves
the shadow map's area by whole texels only, so shadows don't shimmer as it moves (demo 15).
Demo 13 is the simplest use: one caster, one fixed light, one shadow-map sample per fragment
(`demo_013_simple_shadow_shader.frag`). Demo 14's shader averages 3x3 samples for soft edges
(`demo_014_shadows_shader.frag`).

### Several scenes

A `FeatherApp` says how many scenes to draw (`get_num_scenes_to_render`), and gives each with its
own camera (`get_scene_to_render(i)`, `get_camera_to_render_scene(i)`). Scenes are drawn in
order, each sorted as above and through its own camera; the depth buffer is cleared before each
scene after the first, so a later scene is drawn over the earlier ones, whatever their depths.
Each scene keeps its own GPU state (mesh buffers, atlases, pipelines), released when the app
stops drawing it.

For 2D scenes, `OrthographicCamera` shows the XY plane without perspective, looking down -Z
with Y up. Its `ScreenOrigin` puts the origin at the top left, the centre or the bottom left of
the screen; its `ScreenUnits` are pixels, or a fixed screen height in units (the width follows
the window's aspect ratio). Nearer objects (greater z) are drawn over farther ones.

```rust
let camera = OrthographicCamera::new(ScreenOrigin::TopLeft, ScreenUnits::Height(768.0));
// The screen spans x 0..width and y -768..0; get_visible_size() gives the width.
```

The test app's demos draw one scene, unless they override `Demo::scene_count`, `scene_at` and
`camera_at`. Demo 12 draws demo 0's room with a 2D overlay over it.

### Bones and animation

A `Skeleton` is made from a node's subtree: every named node in it is a bone, and its transform at
that moment is the bone's rest pose. An `AnimationClip` (loaded from JSON with
`AnimationClip::load`) has keyframes per bone: a rotation (degrees around X, Y, Z) and an offset
from the rest position. It loops, interpolated smoothly between keys.

```rust
let skeleton = Skeleton::from_nodes(&scene, walker_node)?;
let walk = AnimationClip::load(assets, "demo_008_walker_walk.json")?;
skeleton.check(&walk)?; // every animated bone exists
// every frame:
skeleton.apply(&mut scene, &walk.sample(time))?;
```

```json
{
  "name": "Walk",
  "duration": 1.1,
  "bones": {
    "LeftHip": { "rotation": [[0.0, [25, 0, 0]], [0.55, [-15, 0, 0]]] },
    "Hips":    { "offset":   [[0.0, [0, 0, -0.02]], [0.275, [0, 0, 0.012]]] }
  }
}
```

Each key is `[time, [x, y, z]]`, times in seconds in `0..duration`, in increasing order. The
animation is rigid: meshes hang from bones as child nodes (unnamed, so they aren't bones) and move
as a whole; they don't bend at the joints (there is no mesh skinning). Demo 8 builds a person this
way (`demo_008_walker_scene.json`), and demo 9 a spider (`demo_009_spider_scene.json`).

### Text

Text is drawn with a font atlas: images of characters made by the font creator
([Fonts](#fonts)) and packed by the atlas creator, named after their code points. A
`TextBuilder` lays text out in scene units; a `Text` shows it on a node and updates it when the
text changes:

```rust
let material = create_text_material(&mut scene, None, shader, font_atlas)?; // whole atlas
let builder = TextBuilder::new(font_atlas)
    .height(0.5) // line height in scene units
    .align(HorizontalAlign::Center, VerticalAlign::Center)
    .plane(TextPlane::XY) // or XZ, lying on a floor
    .wrap(None) // or Some(width): break lines between words
    .method(TextMethod::OneMesh); // or NodePerGlyph
let mut clock = Text::create(&mut scene, parent, Some("Clock".into()), material, builder, "")?;
// every frame; updated only when the text changes:
clock.set_text(&mut scene, &format!("{:.1} s", time))?;
```

There are two ways to draw text (`TextMethod`), the two methods of the C++ feather's
`TextBuilder` (`oneMesh` and `treeOfUnits`):

| | `OneMesh` | `NodePerGlyph` |
|---|---|---|
| Nodes | the text's node, transparent, with the mesh | a transparent child per drawn character (`Text::get_glyph_nodes`) |
| Meshes | one per text: a quad per character, the character's rectangle of the atlas in the texture coordinates | one square shared by all glyph nodes; each node's transform places it, its `umin`, `umax`, `vmin`, `vmax` uniforms hold the rectangle |
| Draws | one per text | one per character |
| Changing the text | builds a new mesh, removes the old one (`Scene::remove_mesh`) | reuses the glyph nodes, adding or removing (`Scene::remove_node`) the difference; no meshes |
| Good for | most text | text whose characters move or are coloured one by one |

Both use one material per font, whose texture is the whole atlas. The shader samples the atlas
at `mix(vec2(umin, vmin), vec2(umax, vmax), uv)` and takes the colour's alpha from the font
images, which are white. `create_text_material` sets those uniforms to the whole atlas (0..1),
which is what one-mesh text needs, and glyph nodes override them with their own. For that the
shader must not name them for the engine (`umin_uniform` ... in the scene file), or the engine
would fill them from the material's texture over the nodes' values; `Text::create` checks this.
Uniforms set on a text's node, like a colour, reach its glyph nodes too.

A `\n` starts a new line (a new paragraph, with wrapping); characters missing from the font take
a quarter of the line height and aren't drawn. `HorizontalAlign::Justify` stretches the lines
broken by wrapping to the wrap width, widening their spaces. `Text::remove` removes a text with
its node and mesh. Demo 11's crawl uses wrapped, justified text. Demo 10 shows both methods with one shader (`demo_010_text_shader.frag`, coloured
with a `color` uniform): the wave is a node per glyph, everything else one mesh per text.
[doc/text-performance.md](doc/text-performance.md) compares the two methods' performance.

### Uniforms

Materials and nodes each have a list of named uniforms. Before rendering, the engine computes each
node's final (dynamic) uniforms in three layers, each overriding the previous one by name:

1. the uniforms of the node's material,
2. the uniforms inherited from its ancestor nodes,
3. the node's own uniforms.

A node's own uniforms are passed down to its children; its material's uniforms are not.

### Textures

An atlas is an image with optional metadata (a `.atlas` file) naming rectangles of it. A
`Texture` is a plain value, not a scene object: get it from the atlas, by name or for the whole
atlas, and give it to a material:

```rust
let atlas = scene.get_atlas(atlas_handle).unwrap();
let texture = atlas.get_texture("plate_01").unwrap(); // or atlas.get_full_texture()
scene.material_set_texture(material, Some(texture))?;
```

Every sampler of the material's shader samples the texture's atlas. Shaders always sample the
whole atlas; to use only the texture's rectangle, declare `float` uniforms for its edges, name
them in the shader's `ShaderUniformNames` (`umin`, `umax`, `vmin`, `vmax`, like the `model`,
`view` and `projection` matrices) and map the texture coordinates with them:

```glsl
layout(binding = 1) uniform sampler2D atlas;
layout(binding = 2) uniform TextureRect { float umin; float umax; float vmin; float vmax; } rect;
...
outColor = texture(atlas, mix(vec2(rect.umin, rect.vmin), vec2(rect.umax, rect.vmax), uv));
```

`v` is 0 at the top row of the atlas image. A shader with samplers whose material has no texture
shows magenta, and a warning is logged.

When drawing, the engine adds the node's global transform, the camera's view and projection and
the edges of the material's texture under the names given to `create_shader`, then writes every value into the shader's uniform
blocks and push constants by name, at the offsets read from the shader's SPIR-V. A uniform with
no value, a value of the wrong type, or a value the shader doesn't use is logged once as a
warning.

## Scene files

`Scene::load(assets, "name.scene.json")` builds a scene from a JSON file. The full format is
described in [doc/scene-format.md](doc/scene-format.md). The file lists the
atlases, meshes, shaders and materials, and describes the node tree from the root.
Objects refer to each other by name; files are asset names read through the `AssetsProvider`,
so the scene file holds no binary data. The viking room:

```json
{
  "atlases":  [{ "name": "Room atlas", "bitmap": "demo_000_viking_room_atlas.png" }],
  "meshes":   [{ "name": "Room", "file": "demo_000_viking_room_mesh.obj" }],
  "shaders":  [{
    "name": "Basic",
    "vertex": "demo_000_viking_room_shader.vert.spv", "fragment": "demo_000_viking_room_shader.frag.spv",
    "model_uniform": "model", "view_uniform": "view", "projection_uniform": "proj"
  }],
  "materials": [{ "name": "Room", "shader": "Basic", "texture": { "atlas": "Room atlas" } }],
  "root": {
    "name": "Scene root",
    "children": [{ "name": "Room", "mesh": "Room", "material": "Room" }]
  }
}
```

| Item | Fields |
| ---- | ------ |
| atlas | `name`, `bitmap` (PNG), optional `metadata` (`.atlas`) |
| mesh | `name`, and one of `file` (OBJ), `cuboid` (optional `x`, `y`, `z` as `[min, max]`; `{}` is a 1×1×1 cube), `plane` (optional `x`, `y` as `[min, max]` and `segments` `[columns, rows]`) or `sphere` (optional `radius`, `segments`, `rings`; `{}` is a ball of diameter 1) |
| shader | `name`, `vertex`, `fragment` (SPIR-V), optional `model_uniform`, `view_uniform`, `projection_uniform`, `umin_uniform`, `umax_uniform`, `vmin_uniform`, `vmax_uniform` |
| material | `name`, `shader`, optional `texture` (`{ "atlas": ..., "name": ... }`; without `name` the whole atlas), optional `uniforms` |
| node | all optional: `name`, `mesh`, `material`, `transform`, `visible`, `transparent`, `uniforms`, `children` |

- Names must be unique within each list. Nodes are found with `Scene::find_node(name)`.
- Uniform values are written as `{ "<glsl type>": value }`: `float`, `int`, `uint`, `bool`,
  `vec2`–`vec4`, `ivec2`–`ivec4`, `uvec2`–`uvec4` (arrays) or `mat2`–`mat4` (column-major arrays).
- `transform` is either `matrix` (16 numbers, column-major) or any of `translation` `[x, y, z]`,
  `rotation` `[x, y, z]` in degrees (around X, then Y, then Z) and `scale` (a number or
  `[x, y, z]`).
- Unknown fields, duplicate names and unknown names are errors; the message says where, e.g.
  ``node `Scene root`: node `Room`: unknown mesh `Rom` ``.

## Roadmap

- Render more than one scene
- Android support (including an Android `AssetsProvider`)
- Separate the engine into a library crate
