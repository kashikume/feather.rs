# feather.rs

A lightweight 3D engine written in Rust on top of the Vulkan API.

Targets Windows and Android, with other platforms possible later.

> **Status: early development.** The project is being turned from a Vulkan tutorial-style example
> into the basic parts of an engine. APIs, file layout and module names change often.

## Features

What works today:

- Vulkan renderer (via [`vulkanalia`](https://github.com/KyleMayes/vulkanalia)) with swapchain recreation on resize
- Windowing and event loop via [`winit`](https://github.com/rust-windowing/winit)
- Scene graph made of nodes, with meshes attached to nodes
- Mesh builders: Wavefront `.obj` loading (via `tobj`) and procedural cuboids
- Perspective camera
- Textures (PNG) with mipmaps and MSAA
- Vulkan validation layers turned on automatically in debug builds

## Requirements

- **Rust** (stable, edition 2021): <https://rustup.rs>
- **Vulkan SDK**: <https://vulkan.lunarg.com>. You need it for:
  - `glslc`, which the build uses to compile the GLSL shaders to SPIR-V
  - `VK_LAYER_KHRONOS_validation`, which debug builds **require**. Without it, start-up fails with
    `Validation layer requested but not supported.`
- A GPU and driver that support Vulkan

## Building

Run from the repository root, because assets are loaded from `resources/` using relative paths.

```powershell
cargo run              # debug build, validation layers on
cargo run --release    # release build, validation layers off
```

The development app (`src/testapp`) opens a 1024×768 window and renders the model in
`resources/viking_room.obj`.

### Shaders

`build.rs` compiles every shader in `shaders/` (`.vert`, `.frag`, `.comp`, `.geom`, `.tesc`,
`.tese`) with `glslc` during `cargo build`, and recompiles whenever anything in `shaders/` changes.
Release builds pass `-O` to `glslc`. The output goes to Cargo's `OUT_DIR`, keeps the same relative
path and adds `.spv`, so `shaders/shader.vert` becomes `shaders/shader.vert.spv`. Embed a
compiled shader with:

```rust
include_bytes!(concat!(env!("OUT_DIR"), "/shaders/shader.vert.spv"))
```

`build.rs` looks for `glslc` in this order: the `GLSLC` environment variable,
`$VULKAN_SDK/Bin/glslc`, then `glslc` on `PATH`.

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
    scene.rs         scene graph (nodes, meshes, mesh buffers)
    mesh*.rs         meshes and mesh builders (.obj file, cuboid)
    camera.rs, perspectivecamera.rs
    instance.rs, physicaldevice.rs, logicaldevice.rs, swapchain.rs, pipeline.rs, ...
                     Vulkan object wrappers
  testapp/           development application used to exercise the engine
shaders/             GLSL sources (shader.vert, shader.frag), compiled by build.rs
build.rs             compiles shaders to SPIR-V at build time
resources/           models and textures
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

`src/testapp/testapp.rs` is a minimal example. It builds a scene with a root node and a room node,
loads a mesh from an `.obj` file and sets up a perspective camera. `main.rs` passes it to the engine:

```rust
let mut app = unsafe { App::create(&window, Box::new(TestApp::new()))? };
app.run(&window, event_loop)?;
```

## Roadmap

- Finish moving rendering onto the scene graph (per-node meshes and mesh buffers)
- Materials and multiple textures
- Android support
- Separate the engine into a library crate
