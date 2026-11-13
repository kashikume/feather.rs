# Text performance improvements

Proposals following the measurements in [../text-performance.md](../text-performance.md). Most
help any large or changing scene, not only text. Done items are in
[../archive/done-text-performance-improvements.md](../archive/done-text-performance-improvements.md).

Ordered by expected gain for the cost of the change.

## 1. Release removed GPU mesh buffers without waiting for the device

**Problem**: removing a mesh buffer makes `sync_gpu_state` call `device_wait_idle`, 3–4 ms in
every frame where a text changes, whatever its size.

**Proposal**: instead of waiting, queue the removed `GpuMeshBuffer` with the frame it was last
used in, and destroy it once that frame's fence has signalled (after `MAX_FRAMES_IN_FLIGHT`
frames). Destroy everything still queued in `release_scene_gpu_state` and `destroy`, after their
existing waits.

**Also**: `GpuMeshBuffer::upload` waits for its staging copy. Uploads could be recorded into a
command buffer submitted before the frame's draws, with the staging buffers freed through the
same deferred queue, so nothing waits.

**Check**: a one-mesh text changing every frame should cost about what the text's layout costs
(0.03 ms for 20 characters) instead of 3.9 ms of sync.

## 2. Update only the nodes that changed in `update_nodes`

**Problem**: `Scene::update_nodes` recomputes the uniforms and global transform of every node
every frame: 8.3 ms for 2,043 nodes, even when nothing changed.

**Proposal**: mark a node dirty when its transform, uniforms, material or parent change (and
when its material's uniforms change), and recompute only dirty subtrees. This needs the node and
material setters to go through the scene, or a change counter on nodes and materials that
`update_nodes` compares with the one it last saw.

**Check**: static scenes should spend close to nothing in prepare, whatever their number of
nodes.

## 3. Fewer allocations per draw in `collect_draws`

**Problem**: every draw allocates small vectors (sampler atlases, engine uniforms, texture views,
uniform block offsets, push constants) and looks every shader member up by name: 6.4 ms for 2,000
draws.

**Proposal**:

- Precompute, per shader, the list of members to fill and where each value comes from (engine
  value or uniform id), once, when the shader's pipeline is created.
- Reuse buffers across draws and frames (`SmallVec`, or one arena per frame for offsets and push
  constants).
- Compute the per-material part (texture views, sampler atlases) once per material per frame.

**Check**: collect for 2,000 glyph nodes should be well under 1 ms.

## 4. Sort less

**Problem**: every transparent node is a sorting item, so each glyph node is sorted on its own
every frame.

**Proposal**: sort only when the camera or a transparent node's transform changed, reusing the
previous order otherwise (it is nearly sorted, so an insertion sort is cheap).

## 5. Text-specific

- `Text::set_text` with `NodePerGlyph` could skip glyph nodes whose character and place didn't
  change.
- One-mesh text that changes often could keep its vertex buffer and overwrite it in place
  (fixed capacity, a host-visible buffer per frame in flight) instead of building a new mesh:
  this needs the renderer to support updating a mesh buffer's contents.
