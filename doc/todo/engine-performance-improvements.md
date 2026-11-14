# Engine performance improvements

Findings from a review of the engine (`crates/feather`) on 2026-11-12: each item says what to
check once it is done. Done items (MSAA store, cheaper MSAA, lazily allocated attachments), with
their measurements, are in
[../archive/done-engine-performance-improvements.md](../archive/done-engine-performance-improvements.md).
Items already planned in [text-performance-improvements.md](text-performance-improvements.md)
are not repeated (removing
mesh buffers without `device_wait_idle`, updating only changed nodes, fewer per-draw allocations
in `collect_draws`, sorting less, text-specific work).

Ordered by expected gain for the cost of the change. File references are relative to
`crates/feather/src/`.

## GPU

### 1. Frustum culling

Planned in [frustum-culling-plan.md](frustum-culling-plan.md).

**Problem**: `Scene::draw_items` (`scene.rs:453`) returns every visible node, so off-screen nodes
are packed, bound and drawn. Shadow casters are all drawn into the shadow map too.

**Proposal**:

- Compute a bounding sphere (or box) per mesh when it is created or loaded; keep it in `Mesh`.
- In `draw_items`, transform it by the node's global transform and test it against the planes of
  the active camera's view-projection; skip nodes outside.
- Test shadow casters against the light's view-projection instead.
- Optionally cull whole subtrees, if nodes get bounds of their subtree.

**Check**: in the city demos, the number of draws and the collect time drop when the camera looks
at part of the city; nothing visible pops at the screen edges.

### 2. Instancing

**Problem**: repeated meshes (the city's cars, buildings, lamps) are separate draws, each with its
own uniform packing and descriptor binding.

**Proposal**: group draws with the same mesh, material and pipeline and issue one instanced draw,
with per-instance data (model matrix, a few uniforms) in a storage or vertex buffer. This changes
how shaders read their per-node values, so it needs a design of its own; do it after culling.

**Check**: draw count and collect time in demo 16.

## Pipelines and swapchain

### 3. Keep pipelines across resizes

**Problem**: viewport and scissor are fixed in each pipeline (`pipeline.rs:487-503`), so
`destroy_swapchain` (`app.rs:789-792`) destroys every pipeline, and they are all recreated at the
next frame: every resize, rotation and Android resume rebuilds them.

**Proposal**: make viewport and scissor dynamic state (the shadow pass already does, see
`record_shadow_pass`) and set them when recording. Keep pipelines while the render pass is
compatible (same color and depth format and sample count); recreate them only when that changes.

**Check**: resizing the window and F9 (suspend/resume) don't log pipeline creation; no hitch after
a resize.

### 4. A pipeline cache

**Problem**: pipelines are created without a cache (`PipelineCache::null()`, `pipeline.rs:567`),
so each first draw of a shader compiles it from scratch, at every start.

**Proposal**: one `vk::PipelineCache` for the device, loaded from a file at start and saved at
`destroy` (on Android, in the app's internal storage). The driver checks that the data matches
the device and ignores it otherwise.

**Check**: time to the first frame on the second start, especially on Android.

## CPU per frame

### 5. Don't rebuild already-reported uniform issues

**Problem**: every draw, every frame, `unused_uniforms` (`uniformdata.rs:246`) makes a `String`
for each node uniform the shader doesn't use, and `collect_scene_draws` (`app.rs:435-441`) clones
and hashes every issue to find it was already reported.

**Proposal**: make `UniformIssue` hold `UniformName` instead of `String` (copy, compare and hash
as integers), turning names into strings only when logging. Better still, check the issues once
per (shader, list of uniform names) and skip the check for draws already seen.

**Check**: collect time for a scene whose nodes carry uniforms their shader doesn't use.

### 6. Fewer descriptor set lookups

**Problem**: `ShaderPipeline::descriptor_sets` builds its key with `textures.to_vec()`
(`pipeline.rs:277`) and returns `cached.sets.clone()` (`pipeline.rs:284`): two allocations and the
hashing of a vector per draw.

**Proposal**: in `upload_uniforms`, reuse the previous draw's sets when its pipeline and textures
are the same (draws are grouped by shader, often by material). Make the key hashable without
allocating (a small fixed array, or a texture-combination id), and return the sets as a slice or a
small inline array.

**Check**: the "uniforms" part of the frame in the text measurement setup.

### 7. Sort keys computed once

**Problem**: `sort_for_rendering` (`scene.rs:57`) computes each transparent item's depth inside
the comparator, a matrix-vector product per comparison. Opaque items are grouped by shader only.

**Proposal**: compute one key per item before sorting (depth for transparent items) and use
`sort_unstable_by_key` where order among equal items doesn't matter. Group opaque items by
(shader, material, mesh buffer) to cut descriptor set and vertex buffer binds.

**Check**: same image; fewer binds per frame (count them in `record_command_buffer`).

### 8. Cheaper hierarchy traversal

**Problem**:

- Node children are a `HashSet<usize>` (`node.rs:13`): slower to walk than a list, and the
  drawing order of siblings is random.
- `update_subtree` (`scene.rs:442-446`) allocates the uniforms passed to children once per node and
  clones them once per child.
- `root_nodes()` scans every node and allocates a list, twice per frame (`update_nodes`,
  `draw_items`).

**Proposal**: children as a `Vec<usize>` (removal is rare); a stack of computed parent uniforms
shared by siblings (index into a per-traversal arena instead of a clone per child); the scene
keeps its list of root nodes up to date in `create_root_node`, `disconnect_node` and
`remove_node`.

**Check**: `text::tests::uniform_benchmark`'s `update_nodes` time.

### 9. Reuse per-frame buffers

**Problem**: the uniform arena (`app.rs:253`), the draw list and the draw items are created
empty every frame and grow by reallocation.

**Proposal**: keep them in `App` (or `SceneGpuState`) and clear them at the start of each frame.
The arena could also be written straight into the mapped uniform buffer, saving the copy in
`upload_uniforms`, once the buffer is chosen before packing.

**Check**: allocation count per frame (for example with a counting global allocator in a test).

### 10. `Object::get_name` without cloning

**Problem**: `Object::get_name` returns `Option<String>`, cloning the name at every call.
`ObjDB::remove` calls it on every object while looking for another one with the same name, so
removing many named objects costs O(n²) allocations.

**Proposal**: return `Option<&str>`. Callers that need a `String` convert explicitly.

**Check**: removing a subtree of 2,000 named nodes.

## Loading and uploads

### 11. Fewer allocations and waits when uploading

**Problem**: every `create_buffer` is its own `vkAllocateMemory` (`buffers.rs:111`), and every
copy waits for the whole queue (`queue_wait_idle` in `end_single_time_commands`, `other.rs:63`).
A mesh buffer takes four allocations and two waits; a texture waits for its copy and again for
its mipmaps. Devices limit the number of allocations (4,096 on many), and each one is slow.
`get_memory_type_index` also queries the memory properties at every call.

**Proposal**:

- A simple sub-allocator: large blocks per memory type, buffers placed inside them.
- One staging buffer and one submission for a mesh buffer's vertices and indices, and for a
  texture's copy and mipmaps; wait on a fence instead of the queue.
- Query the memory properties once, at device creation.

Together with item 1 of [text-performance-improvements.md](text-performance-improvements.md)
(uploads without waiting), this also helps text that changes every frame.

**Check**: load time of demo 16; "sync" time for changing one-mesh text.

## Suggested order

1 (culling) first, then 5–7, which are small and local. 3 and 4 matter most for Android resume
and rotation. 2 and 11 are larger and need their own design.
