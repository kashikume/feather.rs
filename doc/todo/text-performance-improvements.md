# Text performance improvements

Proposals following the measurements in [../text-performance.md](../text-performance.md). Item 3
is done. Most help any large or changing scene, not only text.

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

## 3. Cheaper uniform names (done)

**Problem**: uniforms are looked up and copied by `String` name: cloning inherited uniforms,
building dynamic uniforms, setting glyph rectangles in `Text`, and matching shader members when
packing each draw.

**Proposal**: intern uniform names, giving each name a small id when it is first used
(`UniformName(u32)`, kept in a global or per-scene table). `Uniforms` stores ids and values
without allocations. Shader layouts store the ids of their members, so packing compares
integers. The public API can keep taking `&str`.

**Done**: `UniformName` (`uniformname.rs`) interns names in one table for the process.
`Uniforms` is a `Vec` of (id, value) without a `HashMap`, and nodes refill their inherited and
dynamic lists in place. Shader layout members and samplers are named by `UniformName`, and
`Shader` keeps its engine uniform names as ids (`ShaderUniformIds`). `Text` sets glyph
rectangles by id.

The CPU benchmark `text::tests::uniform_benchmark` (2,000 glyph nodes, no GPU;
`cargo test --release -p feather -- --ignored uniform_benchmark --nocapture`), measured on
2026-11-11, in ms per frame (mean of three runs each):

| | before | after |
|---|---|---|
| `set_text` of all 40 lines (glyph uniforms and transforms) | 0.58 | 0.27 |
| `update_nodes` | 2.98 | 0.41 |
| packing every draw's uniforms | 2.44 | 2.10 |

Packing barely changed: its cost is now mostly the per-draw allocations of item 4. The frame-time
scenarios of [../text-performance.md](../text-performance.md) weren't re-run (they need a
temporary demo that isn't in the repository).

**Check**: setting the four rectangle uniforms of 2,000 glyph nodes (1.8 ms) should get close
to the cost of one-mesh layout, and prepare and collect should drop further.

## 4. Fewer allocations per draw in `collect_draws`

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

## 5. Sort less

**Problem**: every transparent node is a sorting item, so each glyph node is sorted on its own
every frame.

**Proposal**: sort only when the camera or a transparent node's transform changed, reusing the
previous order otherwise (it is nearly sorted, so an insertion sort is cheap).

## 6. Text-specific

- `Text::set_text` with `NodePerGlyph` could skip glyph nodes whose character and place didn't
  change.
- One-mesh text that changes often could keep its vertex buffer and overwrite it in place
  (fixed capacity, a host-visible buffer per frame in flight) instead of building a new mesh:
  this needs the renderer to support updating a mesh buffer's contents.
