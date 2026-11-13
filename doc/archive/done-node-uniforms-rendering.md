# Rendering nodes with their uniforms (done)

Archived on 2026-11-14: steps 1 to 4 are done. Open items are in
[../todo/node-uniforms-rendering.md](../todo/node-uniforms-rendering.md).

Goal: draw every visible node that has a mesh and a material, using the node's
`dynamic_uniforms()` (material, then inherited, then own uniforms) and the shader's matrix uniform
names.

## Where we are

- Scene, nodes, materials, shaders, atlases and uniforms exist on the CPU side; textures are
  values held by materials.
  `Scene::update_nodes` computes the final uniforms and the global transform of each node.
- Step 1 is done: every visible node with a mesh and a material is drawn with its global
  transform as the model matrix (push constant). Command buffers are recorded every frame from
  `Scene::draw_items`.
- Step 2 is done: `Shader::new` reflects the SPIR-V into a `ShaderLayout` (uniform blocks, push
  constants, samplers; `shaderlayout.rs`, using `spirq`). Each shader gets its own Vulkan pipeline
  (`ShaderPipeline`), created when first drawn and recreated with the swapchain.
- Step 3 is done: each draw's uniform blocks and push constants are filled from the node's
  `dynamic_uniforms()` plus model/view/proj (`uniformdata.rs`), stored in one uniform buffer per
  swapchain image, and bound with dynamic offsets through each shader's own descriptor sets.
- Step 4 is done: atlases are uploaded as GPU images with mipmaps (`gpuimage.rs`), every sampler
  is bound to the atlas of the material's `Texture`, and the texture's UV rectangle goes to the
  shader's `umin`/`umax`/`vmin`/`vmax` uniforms. Descriptor sets are cached per (swapchain image,
  textures). The app runs from any directory.
- Several scenes are drawn, one over the other, each with its own camera and its own GPU state
  (`SceneGpuState` in `appdata.rs`, by `Scene::get_id`); the depth buffer is cleared between
  scenes.

## Design

### Descriptor sets

The renderer imposes no set layout on shaders: sets and bindings come from reflection.

- Every uniform block is a `UNIFORM_BUFFER_DYNAMIC` filled per draw. Camera matrices are just
  values injected by name, so they are copied for every node.
- Push constant blocks are filled the same way, from the same values.
- Each shader pipeline owns a descriptor pool with its sets for every swapchain image. Before a
  frame is recorded, the sets of that image are pointed at the frame's uniform buffer.

A convention such as set 0 = per-frame camera data (filled once per frame), set 1 = textures,
set 2 = per-node data would save copying and binding; see "Later".

### Uniform name → GPU location

Reflect each shader's SPIR-V when the `Shader` is created (`spirq`, see `ShaderLayout`):

- uniform blocks: set, binding, size, and for each member its name, offset and type;
- samplers: set, binding, name.

Uniform names are the GLSL member names (`model`, `tint`, `texSampler`). The shader builds its own
descriptor set layouts and pipeline layout from the reflection data.

### Per-node uniform data

For each node with a mesh and a material, each frame:

1. take `dynamic_uniforms()`;
2. add engine-provided values under the names stored in `Shader`: node global transform →
   `uniform_model_name`, camera view → `uniform_view_name`, camera projection → `uniform_proj_name`;
3. pack non-texture values into bytes at the reflected offsets and matrix strides
   (`UniformValue::write_bytes`; `bool` as 4 bytes);
4. append to one large per-frame uniform buffer, each node's block aligned to
   `minUniformBufferOffsetAlignment` (usually 256 on desktop, 16–256 on Android);
5. bind as `UNIFORM_BUFFER_DYNAMIC` and pass the node's offsets at draw time.

Problems (a uniform without value, a value of the wrong type, a value the shader doesn't use)
are logged once per shader and uniform; the uniform keeps zero bytes.

### Textures

- Each atlas gets one GPU image (RGBA8 sRGB, with mipmaps), uploaded when the scene first has
  it. All textures share one sampler (linear, anisotropic, repeat).
- A material has at most one `Texture` (an atlas handle and a UV rectangle, taken from the
  atlas with `Atlas::get_texture(name)` or `get_full_texture()`); every sampler of its shader is
  bound to that atlas. Image views can't crop, so a texture that is part of an atlas reaches the
  shader as a UV rectangle: the engine fills the `float` uniforms named by the shader's
  `ShaderUniformNames` `umin`, `umax`, `vmin`, `vmax`, and the shader maps
  `mix(vec2(umin, vmin), vec2(umax, vmax), uv)`. Full-atlas textures don't need them.
- Samplers of a material without a texture (or whose atlas is gone) get a 1x1 magenta fallback
  image, and the problem is logged.
- Each shader pipeline caches descriptor sets per (swapchain image, image view of each sampler),
  allocated from pools that grow when full. Nodes using the same atlases share sets.

### Recording draws

Command buffers are re-recorded every frame by walking the scene:

```rust
for node in visible nodes with mesh + material (sorted by shader, then material):
    if shader changed   { cmd_bind_pipeline(shader.pipeline) }
    if textures changed { cmd_bind_descriptor_sets(set = 1, texture_set) }
    cmd_bind_descriptor_sets(set = 2, object_set, dynamic_offsets = [node_offset])
    cmd_bind_vertex_buffers / cmd_bind_index_buffer(node's mesh buffers)
    cmd_draw_indexed(...)
```

### Alternative for the model matrix

A push constant (64 bytes per draw, no buffer to manage) is the cheapest way to pass the model
matrix. Push constants are only guaranteed to be 128 bytes, so they can't carry arbitrary
uniforms; the dynamic uniform buffer is still needed for those.

## Tasks

Each step should leave the app running.

### 1. Draw nodes with their own model matrix

- [x] Compute node global transforms (parent global × local), in the same pass as the dynamic
      uniforms (`Scene::update_nodes` / `Scene::update_node`)
- [x] Add a push constant range for the model matrix to the pipeline layout; change
      `demo_000_viking_room_shader.vert` to read `model` from it
- [x] Re-record command buffers every frame
- [x] Walk the scene: for each visible node with a mesh, bind its mesh buffers, push its model
      matrix, draw (`Scene::draw_items`; a hidden node hides its subtree)
- [x] Upload mesh buffers of the app's scenes when they are created
- [x] Remove the hard-coded spinning model matrix from `App::update_uniform_buffer`; spin the room
      node from `TestApp::on_update` instead
- [x] Remove the engine's internal scene and camera (`AppData::scene`, `AppData::camera`,
      `AppData::mesh`, `AppData::mesh_buffer`, `model.rs`); render the app's scene and camera
- [x] Tests for global transforms and draw items

### 2. Shader reflection and a pipeline per shader

- [x] Choose a SPIR-V reflection crate and reflect in `Shader::new` (`spirq`: pure Rust, so it
      also builds for Android)
- [x] Store uniform blocks (set, binding, size, members with name/offset/type), push constants
      and samplers (set, binding, name) in `Shader` (`Shader::get_layout`)
- [x] Check that reflected types are supported by `UniformValue`; fail `Shader::new` otherwise
      (also for unknown or non-`mat4` model/view/proj names, and duplicate uniform names)
- [x] Create descriptor set layouts and the pipeline layout from reflection data
- [x] Create a Vulkan pipeline per shader (cached, recreated with the swapchain)
- [x] Tests for reflection of `demo_000_viking_room_shader.vert` / `.frag`, plus test-only
      shaders in `shaders/tests/` (all uniform types, stage merging, unsupported cases)
- [x] Compile shaders without `-O`, also in release: it strips the names reflection needs

### 3. Uniform values on the GPU

- [x] `UniformValue::write_bytes(&self, buf, offset, matrix_stride)` with tests for every
      variant (the matrix stride comes from reflection: 16 in std140 blocks, 8 for a `mat2` in
      std430 push constants)
- [x] Per-frame dynamic uniform buffer (one per swapchain image), growing when needed
      (`FrameUniformBuffer`)
- [x] Fill each node's blocks from `dynamic_uniforms()` plus model/view/proj by name
      (`uniformdata::pack_draw`); push constants too
- [x] Every uniform block bound as `UNIFORM_BUFFER_DYNAMIC` per node (instead of a set
      convention; see "Descriptor sets")
- [x] Warn (once) about uniforms that the shader doesn't use, about shader uniforms with no
      value, and about values of the wrong type
- [x] Allocate descriptor sets from each shader's own set layouts; remove the fixed
      `create_descriptor_set_layout`, `check_renderer_support`, `UniformBufferObject` and
      `descriptors.rs`

### 4. Textures from atlases

- [x] Upload each atlas as a GPU image (with mipmaps) (`GpuImage`, `App::sync_atlases`)
- [x] Decide how a texture that is part of an atlas reaches the shader: `umin`, `umax`,
      `vmin`, `vmax` uniforms named by the shader (`ShaderUniformNames`)
- [x] Sampler descriptor sets cached per (shader, swapchain image, textures)
- [x] Bind the material's texture to the reflected sampler bindings
      (`uniformdata::sampler_atlases`); fallback texture and warnings when it is missing
- [x] Textures as values from the atlas's `.atlas` metadata (`Atlas::get_texture`), stored in
      materials; no texture objects in the scene
- [x] Remove the hard-coded texture code from `texture.rs` and the
      `resources/demo_000_viking_room_atlas.png` path; the app then runs from any directory

### Done later

- [x] Render more than one scene (per-scene camera data instead of one uniform buffer per frame)
- [x] Sort draws: opaque by shader, transparent back to front (`scene::sort_for_rendering`);
      transparent nodes get a blending pipeline variant without depth writes (`PipelineKey`)
- [x] Transparent nodes drawn after opaque ones, back to front
