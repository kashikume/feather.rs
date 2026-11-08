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
- Scene objects (meshes, nodes, atlases, textures, shaders, materials) stored in the scene and
  referenced by handle
- Mesh builders: Wavefront `.obj` loading (via `tobj`) and procedural cuboids
- Atlases loaded from PNG (any color type, converted to 8-bit RGBA), with textures cut from them
- Shaders (SPIR-V) and materials (a shader plus uniform values, textures included); each
  shader's uniform blocks, push constants and samplers are read from its SPIR-V, and each shader
  gets its own pipeline
- Uniforms on materials and nodes, inherited down the scene graph
- Assets loaded through a platform-specific `AssetsProvider`
- Scenes described in JSON files, with objects referring to each other by name
- Perspective camera
- Textures from atlases, uploaded with mipmaps; textures that are part of an atlas pass their UV
  rectangle to the shader
- MSAA
- Vulkan validation layers turned on automatically in debug builds

> The renderer draws every visible node that has a mesh and a material, with the material's
> shader, the node's uniforms and the textures they name. Only the first scene is drawn. See
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
cargo test             # unit tests
```

The development app (`src/testapp`) opens a 1024×768 window and renders the model in
`resources/viking_room.obj`.

Everything the app loads comes from the `assets/` directory next to the executable, so it can be
started from any directory.

### Shaders

`build.rs` compiles every shader in `shaders/` (`.vert`, `.frag`, `.comp`, `.geom`, `.tesc`,
`.tese`) with `glslc` during `cargo build`, and recompiles whenever anything in `shaders/` changes.
Shaders are compiled without `-O`, also in release builds: optimization strips the debug names
the engine reads uniform names from. A compiled shader keeps its path relative to `shaders/` and
gets `.spv` added, so `shaders/shader.vert` becomes `shader.vert.spv`. It is written to:

- the `assets/` directory next to the executable (`assets/shader.vert.spv`), where applications
  load it through the assets provider as `"shader.vert.spv"` (see [Assets](#assets)),
- `$OUT_DIR/shaders/`, for embedding with
  `include_bytes!(concat!(env!("OUT_DIR"), "/shaders/shader.vert.spv"))`.

`build.rs` looks for `glslc` in this order: the `GLSLC` environment variable,
`$VULKAN_SDK/Bin/glslc`, then `glslc` on `PATH`.

## Assets

`build.rs` copies everything in `resources/` to an `assets/` directory next to the executable
(e.g. `target/debug/assets/`), together with the compiled shaders. It copies
again whenever anything in `resources/` changes.

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

let bytes = assets.get_asset("shader.vert.spv")?;
let mesh = MeshBuilderObjFile::new(&*assets, "viking_room.obj")?.build(&mut scene)?;
let atlas = scene.create_atlas_from_png(&*assets, "viking_room.png", None)?;
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
src/
  main.rs            entry point: creates the window and starts the engine with TestApp
  feather/           the engine
    app.rs           Vulkan setup, main loop, frame rendering
    featherapp.rs    FeatherApp trait implemented by applications
    scene.rs         scene: owns all objects and the node graph
    scenefile.rs     loading a scene from a JSON file
    objdb.rs         handle-based object storage used by the scene
    node.rs          scene graph node (mesh, material, uniforms)
    mesh*.rs         meshes and mesh builders (.obj file, cuboid)
    atlas.rs, texture.rs
                     images loaded from PNG and textures cut from them
    gpuimage.rs      atlases uploaded to the GPU, and the texture sampler
    shader.rs, material.rs
                     SPIR-V shaders and materials
    shaderlayout.rs  uniform blocks, push constants and samplers read from a shader's SPIR-V
    uniform.rs, uniforms.rs
                     uniform values and named uniform lists
    uniformdata.rs   packing uniform values into the bytes a shader reads
    assetsprovider.rs, windowsassetsprovider.rs
                     platform-independent asset access and its Windows implementation
    camera.rs, perspectivecamera.rs
    instance.rs, physicaldevice.rs, logicaldevice.rs, swapchain.rs, pipeline.rs, ...
                     Vulkan object wrappers
  testapp/           development application used to exercise the engine
shaders/             GLSL sources (shader.vert, shader.frag), compiled by build.rs;
                     shaders/tests/ holds shaders used only by unit tests
build.rs             compiles shaders to SPIR-V and copies assets next to the executable
resources/           scene files, models and textures, copied to assets/ by build.rs
```

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
}
```

`src/testapp/testapp.rs` is a minimal example. It loads its scene from
`resources/viking_room.scene.json` (see [Scene files](#scene-files)), finds the room node by name
to spin it in `on_update`, and sets up a perspective camera:

```rust
let assets = create_assets_provider("assets")?;
let scene = Scene::load(&*assets, "viking_room.scene.json")?;
let room_node = scene.find_node("Room").unwrap();
```

Scenes can also be built in code, with `Scene::create_atlas_from_png`, `create_shader`,
`create_material`, `create_node` and the other `Scene` methods.

`main.rs` passes it to the engine:

```rust
let mut app = unsafe { App::create(&window, Box::new(TestApp::new()))? };
app.run(&window, event_loop)?;
```

### Uniforms

Materials and nodes each have a list of named uniforms. Before rendering, the engine computes each
node's final (dynamic) uniforms in three layers, each overriding the previous one by name:

1. the uniforms of the node's material,
2. the uniforms inherited from its ancestor nodes,
3. the node's own uniforms.

A node's own uniforms are passed down to its children; its material's uniforms are not.

### Textures

A `sampler2D` uniform takes a `UniformValue::Texture(handle)`; the sampler is bound to that
texture's atlas. A texture can cover a rectangle of its atlas (`Texture::new`). Shaders always
sample the whole atlas, so to use only the texture's part, declare a `vec4` uniform named after
the sampler plus `Rect` and map the texture coordinates with it:

```glsl
layout(binding = 1) uniform sampler2D texSampler;
layout(binding = 2) uniform Material { vec4 texSamplerRect; } material;
...
outColor = texture(texSampler, material.texSamplerRect.xy + uv * material.texSamplerRect.zw);
```

The engine fills `texSamplerRect` with (u_min, v_min, u_range, v_range). A sampler without a
valid texture shows magenta, and a warning is logged.

When drawing, the engine adds the node's global transform and the camera's view and projection
under the names given to `create_shader`, then writes every value into the shader's uniform
blocks and push constants by name, at the offsets read from the shader's SPIR-V. A uniform with
no value, a value of the wrong type, or a value the shader doesn't use is logged once as a
warning.

## Scene files

`Scene::load(assets, "name.scene.json")` builds a scene from a JSON file. The full format is
described in [doc/scene-format.md](doc/scene-format.md). The file lists the
atlases, textures, meshes, shaders and materials, and describes the node tree from the root.
Objects refer to each other by name; files are asset names read through the `AssetsProvider`,
so the scene file holds no binary data. The viking room:

```json
{
  "atlases":  [{ "name": "Room atlas", "file": "viking_room.png" }],
  "textures": [{ "name": "Room texture", "atlas": "Room atlas" }],
  "meshes":   [{ "name": "Room", "file": "viking_room.obj" }],
  "shaders":  [{
    "name": "Basic", "vertex": "shader.vert.spv", "fragment": "shader.frag.spv",
    "model_uniform": "model", "view_uniform": "view", "projection_uniform": "proj"
  }],
  "materials": [{
    "name": "Room", "shader": "Basic",
    "uniforms": { "texSampler": { "texture": "Room texture" } }
  }],
  "root": {
    "name": "Scene root",
    "children": [{ "name": "Room", "mesh": "Room", "material": "Room" }]
  }
}
```

| Item | Fields |
| ---- | ------ |
| atlas | `name`, `file` (PNG) |
| texture | `name`, `atlas`, optional `rect` `[u_min, v_min, u_range, v_range]`; without `rect` it covers the whole atlas and becomes its full texture |
| mesh | `name`, `file` (OBJ) |
| shader | `name`, `vertex`, `fragment` (SPIR-V), optional `model_uniform`, `view_uniform`, `projection_uniform` |
| material | `name`, `shader`, optional `uniforms` |
| node | all optional: `name`, `mesh`, `material`, `transform`, `visible`, `uniforms`, `children` |

- Names must be unique within each list. Nodes are found with `Scene::find_node(name)`.
- Uniform values are written as `{ "<glsl type>": value }`: `float`, `int`, `uint`, `bool`,
  `vec2`–`vec4`, `ivec2`–`ivec4`, `uvec2`–`uvec4` (arrays), `mat2`–`mat4` (column-major arrays),
  or `texture` with a texture name.
- `transform` is either `matrix` (16 numbers, column-major) or any of `translation` `[x, y, z]`,
  `rotation` `[x, y, z]` in degrees (around X, then Y, then Z) and `scale` (a number or
  `[x, y, z]`).
- Unknown fields, duplicate names and unknown names are errors; the message says where, e.g.
  ``node `Scene root`: node `Room`: unknown mesh `Rom` ``.

## Roadmap

- Render more than one scene
- Android support (including an Android `AssetsProvider`)
- Separate the engine into a library crate
