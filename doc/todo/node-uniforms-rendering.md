# Rendering nodes with their uniforms

Goal: draw every visible node that has a mesh and a material, using the node's
`dynamic_uniforms()` (material, then inherited, then own uniforms) and the shader's matrix uniform
names.

## Where we are

- Scene, nodes, materials, shaders, atlases, textures and uniforms exist on the CPU side.
  `Scene::update_nodes` computes the final uniforms and the global transform of each node.
- Step 1 is done: every visible node with a mesh and a material is drawn with its global
  transform as the model matrix (push constant). Command buffers are recorded every frame from
  `Scene::draw_items`.
- Step 2 is done: `Shader::new` reflects the SPIR-V into a `ShaderLayout` (uniform blocks, push
  constants, samplers; `shaderlayout.rs`, using `spirq`). Each shader gets its own Vulkan pipeline
  (`ShaderPipeline`), created when first drawn and recreated with the swapchain. The model matrix
  is pushed at the offset of the shader's model uniform.
- Still hard-coded in the renderer:
  - one descriptor set shared by all shaders: binding 0 = `UniformBufferObject { view, proj }`,
    binding 1 = the texture (`create_descriptor_set_layout`, `descriptors.rs`).
    `check_renderer_support` refuses shaders with another layout (their nodes are not drawn);
  - view/proj come from the first scene's camera; only the first scene is drawn;
  - one texture loaded directly from `resources/viking_room.png` (`texture.rs`);
  - material and node uniform values are not sent to the GPU.

## Design

### Descriptor set layout convention

| Set | Changes             | Contents                                                     |
| --- | ------------------- | ------------------------------------------------------------ |
| 0   | once per frame      | camera (view, proj); later lights, time                      |
| 1   | per material/textures | samplers                                                   |
| 2   | per node (dynamic offset) | model matrix and all other non-texture uniforms        |

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
3. pack non-texture values into bytes at the reflected offsets using std140 rules
   (`vec3` aligned to 16, `mat3` columns padded to `vec4`, `bool` as 4 bytes, ...);
4. append to one large per-frame uniform buffer, each node's block aligned to
   `minUniformBufferOffsetAlignment` (usually 256 on desktop, 16–256 on Android);
5. bind as `UNIFORM_BUFFER_DYNAMIC` and pass the node's offset at draw time.

### Textures

- Each atlas gets a GPU image; textures are views/samplers of it, or the atlas plus a UV rectangle
  passed as a uniform.
- Descriptor sets for samplers are cached per (shader, set of textures). Nodes that don't override
  textures reuse their material's set.

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
- [x] Add a push constant range for the model matrix to the pipeline layout; change `shader.vert`
      to read `model` from it
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
- [x] Tests for reflection of `shader.vert` / `shader.frag`, plus test-only shaders in
      `shaders/tests/` (all uniform types, stage merging, unsupported cases)
- [x] Compile shaders without `-O`, also in release: it strips the names reflection needs

### 3. Uniform values on the GPU

- [ ] `UniformValue::write_std140(&self, buf, offset)` with tests for every variant
- [ ] Per-frame dynamic uniform buffer (one per frame in flight), growing when needed
- [ ] Fill each node's block from `dynamic_uniforms()` plus model/view/proj by name
- [ ] Set 0 for per-frame camera data; set 2 with `UNIFORM_BUFFER_DYNAMIC` per node
- [ ] Warn (once) about uniforms that the shader doesn't use, and about shader uniforms with no
      value
- [ ] Allocate descriptor sets from each shader's own set layouts; remove the fixed
      `create_descriptor_set_layout` and `check_renderer_support`

### 4. Textures from atlases

- [ ] Upload each atlas as a GPU image (with mipmaps)
- [ ] Decide how a texture that is part of an atlas reaches the shader (UV rectangle uniform or
      separate view)
- [ ] Sampler descriptor sets cached per (shader, textures)
- [ ] Bind `Texture(handle)` uniforms to the reflected sampler bindings
- [ ] Remove the hard-coded texture code from `texture.rs` and the `resources/viking_room.png`
      path; the app then runs from any directory

### Later

- [ ] Render more than one scene (per-scene camera data instead of one uniform buffer per frame)
- [ ] Sort draws by shader and material; skip redundant binds
- [ ] Transparent nodes drawn after opaque ones, back to front
- [ ] Re-upload uniforms only for nodes that changed
