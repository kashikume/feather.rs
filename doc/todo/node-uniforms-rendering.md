# Rendering nodes with their uniforms

Goal: draw every visible node that has a mesh and a material, using the node's
`dynamic_uniforms()` (material, then inherited, then own uniforms) and the shader's matrix uniform
names.

## Where we are

- Scene, nodes, materials, shaders, atlases, textures and uniforms exist on the CPU side.
  `Scene::update_dynamic_uniforms` computes the final uniforms of each node.
- The renderer ignores all of that. It has:
  - one hard-coded descriptor set layout: binding 0 = `UniformBufferObject { model, view, proj }`,
    binding 1 = `texSampler` (`pipeline.rs`, `descriptors.rs`);
  - one uniform buffer per swapchain image, filled in `App::update_uniform_buffer` with a
    hard-coded spinning model matrix;
  - one texture loaded directly from `resources/viking_room.png` (`texture.rs`);
  - command buffers recorded once at start-up (and on resize) that draw a single mesh
    (`commandbuffers.rs`).
- `Node::transform` and `Node::global_transform` exist but are never computed.

## Design

### Descriptor set layout convention

| Set | Changes             | Contents                                                     |
| --- | ------------------- | ------------------------------------------------------------ |
| 0   | once per frame      | camera (view, proj); later lights, time                      |
| 1   | per material/textures | samplers                                                   |
| 2   | per node (dynamic offset) | model matrix and all other non-texture uniforms        |

### Uniform name → GPU location

Reflect each shader's SPIR-V when the `Shader` is created (e.g. `spirq` or `spirv-reflect`):

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

- [ ] Compute node global transforms (parent global × local), in the same pass as
      `update_dynamic_uniforms`
- [ ] Add a push constant range for the model matrix to the pipeline layout; change `shader.vert`
      to read `model` from it
- [ ] Re-record command buffers every frame
- [ ] Walk the scene: for each visible node with a mesh, bind its mesh buffers, push its model
      matrix, draw
- [ ] Remove the hard-coded spinning model matrix from `App::update_uniform_buffer`; spin the room
      node from `TestApp::on_update` instead
- [ ] Remove `AppData::mesh` / `AppData::mesh_buffer` and `model.rs` `load_model`
- [ ] Tests for global transforms

### 2. Shader reflection and a pipeline per shader

- [ ] Choose a SPIR-V reflection crate and reflect in `Shader::new`
- [ ] Store uniform blocks (set, binding, size, members with name/offset/type) and samplers
      (set, binding, name) in `Shader`
- [ ] Check that reflected types are supported by `UniformValue`; fail `Shader::new` otherwise
- [ ] Create descriptor set layouts and the pipeline layout from reflection data
- [ ] Create a Vulkan pipeline per shader (cached, recreated with the swapchain)
- [ ] Tests for reflection of `shader.vert` / `shader.frag`

### 3. Uniform values on the GPU

- [ ] `UniformValue::write_std140(&self, buf, offset)` with tests for every variant
- [ ] Per-frame dynamic uniform buffer (one per frame in flight), growing when needed
- [ ] Fill each node's block from `dynamic_uniforms()` plus model/view/proj by name
- [ ] Set 0 for per-frame camera data; set 2 with `UNIFORM_BUFFER_DYNAMIC` per node
- [ ] Warn (once) about uniforms that the shader doesn't use, and about shader uniforms with no
      value

### 4. Textures from atlases

- [ ] Upload each atlas as a GPU image (with mipmaps)
- [ ] Decide how a texture that is part of an atlas reaches the shader (UV rectangle uniform or
      separate view)
- [ ] Sampler descriptor sets cached per (shader, textures)
- [ ] Bind `Texture(handle)` uniforms to the reflected sampler bindings
- [ ] Remove the hard-coded texture code from `texture.rs` and the `resources/viking_room.png`
      path; the app then runs from any directory

### Later

- [ ] Sort draws by shader and material; skip redundant binds
- [ ] Transparent nodes drawn after opaque ones, back to front
- [ ] Re-upload uniforms only for nodes that changed
