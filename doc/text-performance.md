# Text performance: one mesh vs a node per glyph

Measured on 2026-11-10, comparing the two `TextMethod`s of `crates/feather/src/text.rs`. See
[todo/text-performance-improvements.md](todo/text-performance-improvements.md) for what could be
done about the findings.

## Summary

- **One mesh per text (`OneMesh`) is much cheaper to draw.** 2,000 static glyphs cost nothing
  measurable above an empty scene.
- **A node per glyph (`NodePerGlyph`) is CPU-bound by the engine's per-node work**, about
  7.5 µs per glyph node per frame even when nothing changes, so 2,000 glyphs take the frame from
  4.2 ms to 17 ms. The GPU is mostly idle.
- **Changing one-mesh text makes the CPU wait for the GPU**, 3–4 ms per frame with any change,
  regardless of how much text changes: removing a mesh buffer waits for the device to go idle
  and uploading a new one waits for the copy.
- **Updating glyph nodes is slower than rebuilding meshes** (1.8 ms vs 1.0 ms for 2,000 glyphs):
  setting uniforms by name costs more than generating the vertices.

Use one mesh by default, and a node per glyph only for short texts whose letters move or are
coloured one by one (demo 10's wave: about 40 nodes, roughly 0.3 ms per frame).

## Setup

- Release build (`cargo build --release`) of the test app, at commit `a0b0168` plus the then
  uncommitted text work.
- Snapdragon X Elite (X1E78100) with an Adreno X1-85 GPU, Windows 11; 1024×768 window, MSAA.
  The swapchain uses MAILBOX, so frames are not capped by vsync.
- A temporary demo (not in the repository) with demo 10's scene, shader and regular font:
  40 lines × 50 characters, 2,000 glyphs, filling the window. "Changing" runs replace every
  line's text every frame with a string of the same length (all characters change).
- Each run: 2 s of warm-up, then 6 s measured. Frame time is the time between `update` calls;
  `set_text` time is measured around the demo's calls; the breakdown comes from temporary
  timers in `App::render`. Each configuration ran once, so treat differences below about 0.5 ms
  as noise.

## Results

### Frame time

| Scenario | Frame time (mean) | FPS | Frame p95 | `set_text` per frame |
|---|---|---|---|---|
| Empty scene (1 character), reference | 4.17 ms | 240 | 6.98 ms | – |
| One mesh, static | 4.17 ms | 240 | 6.48 ms | – |
| Node per glyph, static | 17.40 ms | 57 | 28.55 ms | – |
| One mesh, all 40 lines changing every frame | 5.26 ms | 190 | 6.19 ms | 1.03 ms |
| Node per glyph, all 40 lines changing every frame | 19.57 ms | 51 | 32.89 ms | 1.81 ms |
| One mesh, one 20-character text changing every frame | 4.61 ms | 217 | 5.62 ms | 0.03 ms |
| Node per glyph, one 20-character text changing every frame | 4.17 ms | 240 | 6.71 ms | 0.05 ms |

About 4.2 ms is this machine's floor: in the empty scene almost all of it is spent presenting
and waiting for the GPU.

### Where the time goes

Milliseconds per frame, by part of `App::render`:

| Scenario | update | prepare | sync | fence + acquire | collect | uniforms | record | submit + present |
|---|---|---|---|---|---|---|---|---|
| Empty scene | 0.00 | 0.04 | 0.00 | 0.66 | 0.03 | 0.00 | 0.19 | 3.01 |
| One mesh, static | 0.00 | 0.18 | 0.00 | 0.92 | 0.27 | 0.02 | 0.19 | 2.34 |
| Node per glyph, static | 0.00 | 8.27 | 0.00 | 0.54 | 6.41 | 0.50 | 1.00 | 0.34 |
| One mesh, 40 lines changing | 1.03 | 0.15 | 3.27 | 0.02 | 0.22 | 0.02 | 0.12 | 0.30 |
| Node per glyph, 40 lines changing | 1.81 | 8.43 | 0.00 | 0.50 | 6.57 | 0.51 | 1.02 | 0.35 |
| One mesh, one text changing | 0.03 | 0.02 | 3.91 | 0.03 | 0.02 | 0.00 | 0.11 | 0.35 |
| Node per glyph, one text changing | 0.05 | 0.16 | 0.00 | 0.80 | 0.15 | 0.01 | 0.18 | 2.60 |

- **update**: the app's `on_update`, here the `set_text` calls.
- **prepare**: `Scene::build_missing_mesh_buffers` and `Scene::update_nodes`.
- **sync**: `sync_gpu_state`, which uploads new mesh buffers and atlases and releases removed
  ones.
- **fence + acquire**: waiting for the frame's fence and acquiring the swapchain image.
- **collect**: `collect_draws`, which lists, sorts and packs the draws.
- **uniforms**: `upload_uniforms`.
- **record**: recording the command buffer.
- **submit + present**: submitting and presenting, including waiting when the GPU is behind.

## Findings

### A node per glyph: the engine's per-node work

With 2,043 nodes the frame is CPU-bound: the time spent waiting for the GPU drops from about
3 ms to under 1 ms, while prepare and collect grow to 15 ms.

- **prepare (8.3 ms, about 4 µs per node)**: `update_nodes` recomputes every node's uniforms
  and global transform every frame, whether anything changed or not. Each node clones the
  uniforms inherited from its parent and builds its dynamic uniforms by name. Uniform names are
  `String`s, so a glyph node (4 material uniforms, the inherited colour, 4 own uniforms) makes
  around 20 small allocations per frame.
- **collect (6.4 ms, about 3 µs per draw)**: for every draw, `collect_draws` allocates small
  vectors (sampler atlases, engine uniforms, texture views, uniform offsets, push constants),
  looks every shader member up by name among the engine's and the node's uniforms, and packs
  the values. `sort_for_rendering` sorts the 2,000 transparent items back to front.
- **record (1.0 ms)**: 2,000 draw commands, each binding descriptor sets with its own dynamic
  offsets and pushing constants.
- **uniforms (0.5 ms)**: copying the packed uniform data of 2,000 draws.

Driver overhead is not the main cost: recording and submitting are 1.3 ms of the 15 ms.

### One mesh: waiting for the GPU when text changes

Every frame in which a text changes, sync costs 3–4 ms, as much for one 20-character text as for
40 lines:

- `Scene::remove_mesh` removes a mesh buffer once its last mesh is gone, and `sync_gpu_state`
  then calls `device_wait_idle` before destroying the GPU buffer, because frames still in flight
  may use it.
- `GpuMeshBuffer::upload` copies through a staging buffer and waits for the copy to finish.

In these runs the wait mostly replaced time that would otherwise be spent waiting at present,
so the frame time grew only from 4.2 to 4.6–5.3 ms. In a scene where the GPU has real work, the
same wait would stop the CPU from preparing the next frame while the GPU draws the current one.

Demo 10's clock changes ten times a second, so it pays this about ten times a second.

### Updating text

- One mesh: laying out and generating vertices for 40 × 50 characters takes 1.0 ms.
- A node per glyph: 1.8 ms for the same text, without building any mesh. Each glyph node gets
  a new transform and four uniforms set by name (`Uniforms::set` with `String` names).

So a node per glyph is not cheaper to update in feather.rs, unlike in the C++ feather (where the
appeal of the tree of units is that it needs no new geometry): here its update cost is in the
uniforms, and its draw cost is paid every frame.
