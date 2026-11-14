# Frustum culling: plan

Goal: **don't pack, bind and draw nodes that can't be seen**: nodes outside the active camera's
view are left out of the scene's draws, and shadow casters outside the shadow light's view are
left out of the shadow pass.

Covers item 1 of [engine-performance-improvements.md](engine-performance-improvements.md).
File references are relative to `crates/feather/src/`.

## What the C++ engine does

Nothing: neither the C++ feather (`guitarprompter/contrib/feather`) nor GuitarPrompter has
bounding volumes or culling of any kind. The only "cull" in the C++ code is back-face culling
(`glCullFace`), which feather.rs already does in the pipeline. So there is nothing to port and
no C++ behaviour to keep; this is new work.

## What feather.rs does today

- `Mesh` (`mesh.rs`) keeps its vertices (`Vec<Vertex>`, position, normal, UV) and indices on
  the CPU after upload, and never changes them: text and other dynamic geometry make a new mesh
  and drop the old one (`text.rs`, `set_text`). So bounds computed once per mesh stay valid.
- Animation is rigid (`skeleton.rs`): meshes move with their bone node as a whole, so a mesh's
  bounds transformed by its node's global transform still enclose it. No vertex skinning.
- `Scene::draw_items` (`scene.rs:453`) walks the visible nodes and lists every one with a
  material, an existing shader and a mesh in a mesh buffer, with its global transform.
- `App::collect_scene_draws` (`app.rs:270`) sorts the items (`sort_for_rendering`), takes the
  **casters out of the same list** for the shadow pass, then packs uniforms and makes a
  `GpuDraw` for each item.
- Cameras are perspective or orthographic; `get_projection` already includes the Vulkan
  correction (y down, depth 0..1). `ShadowLight::view_projection` is an orthographic
  view-projection with the same correction (`shadow.rs`).
- There is no draw counter or timing in the engine: the earlier performance items were
  measured with a temporary module that was not committed.

What culling would act on in the demos:
- **Demo 15** (procedural city): the static city is merged into *a few big meshes, one per
  texture* (`demo_015_city.rs`, comment above `add_static`), each spanning the whole city, so
  culling never removes them. Only cars, people (body + two legs each) and traffic-light lamps
  are separate nodes.
- **Demo 16** (OBJ city): statics are one node per model, plus cars, people and lamps, so
  culling removes more here.
- **Text** (e.g. demo 11, crawl, and the text tests with `LINES * 50` glyph nodes): text off
  screen is a large share of the nodes, a good case for culling.

## Proposal

### Bounds: a local axis-aligned box per mesh

- `Mesh` gets `bounds: Option<Aabb>` (min and max corner in the mesh's own space), computed in
  `Mesh::new` from the vertex positions. `None` for a mesh without vertices (it draws nothing).
  `Mesh::get_bounds()` returns it.
- A new small module `bounds.rs` holds `Aabb` and `Frustum`.
- A box rather than a sphere: buildings, roads, text lines and planes are long and flat, and a
  sphere around them is much bigger than the box (more false "visible"). The test below costs
  about the same as a sphere's.

### The test: a transformed box against six planes

- `Frustum::from_matrix(view_projection)` extracts six planes (Gribb–Hartmann) for Vulkan's
  clip space: `-w ≤ x ≤ w`, `-w ≤ y ≤ w`, `0 ≤ z ≤ w`. It works for perspective and
  orthographic matrices alike (camera, UI scenes, the shadow light), and the planes are
  normalised.
- For a node: the box's centre `c` and half-extents `h` in mesh space, model matrix `M` (global
  transform). World centre `M·c`; for each plane with normal `n`, the box's reach along `n` is
  `Σ |n · M.col(i)| · h[i]` (i = x, y, z). The node is outside when `n · (M·c) + d < -reach` for
  any plane. This handles rotation and non-uniform scale exactly (it is the oriented box, not
  the box of the box) and is conservative: no visible node is ever rejected, so nothing pops at
  the screen edges.
- Cost: 6 planes × a few multiply-adds per node; small next to packing uniforms.

### Where: in `draw_items`, with two answers per item

The camera and the shadow light need **separate** answers: a caster outside the camera's view
can still throw a shadow into it, so today's "casters come from the drawn items" must change.

- `Scene::draw_items(&self, culling: Option<&Culling>)`, where `Culling` holds the camera's
  `Frustum` and, when the scene has a shadow light, the light's `Frustum`.
- Each `DrawItem` gets `in_view: bool`; `casts_shadow` becomes "casts a shadow **and** is in
  the light's view". Items that are neither are not returned. With `None`, everything is
  `in_view` (today's behaviour; keeps the existing tests and callers simple).
- `collect_scene_draws` builds `Culling` from `proj * view` and `light.view_projection`, takes
  casters from all returned items (as now, filtered by `casts_shadow`), then keeps only
  `in_view` items for the main pass **before** sorting, so sorting gets cheaper too.

### Opting out

- `Node::set_culled(false)` (default `true`; scene files `"culled": false`) for a node whose
  shader moves vertices beyond the mesh (a vertex shader that displaces, billboards, a sky dome
  that a shader pins to the camera). Such a node is always `in_view` and, if a caster, always in
  the light's view.
- Nodes without bounds (empty mesh) are skipped as today: they draw nothing.

### Counting what is drawn

To check the result (and for later items), the engine keeps per-frame counts: nodes listed,
culled by the camera, drawn, shadow draws, and the time `collect_draws` took. `App` logs them
at `debug` level about once a second (`RUST_LOG=feather=debug`; on Android via logcat), and
passes them to the app every frame (`FeatherApp::on_frame_stats`), so the testapp HUD shows
them on screen. Cheap enough to keep committed, unlike the temporary GPU-timestamp module.

### Not now: subtree bounds

Culling whole subtrees needs each node's bounds of its subtree, updated bottom-up in
`update_nodes` whenever anything below moves. In the demos the big groups ("Cars", "People")
move every frame and spread over the whole city, so their subtree bounds would cover most of
the view and rarely cull. Left for later, if a scene with many static, clustered subtrees shows
the per-node test as a cost.

## Steps

1. `bounds.rs`: `Aabb` (from points, centre, half-extents) and `Frustum` (from a matrix, test
   of a box under a matrix). Unit tests: boxes inside, outside each plane, straddling a plane,
   rotated and non-uniformly scaled; perspective and orthographic matrices with the Vulkan
   correction; a box behind the camera is outside.
2. `Mesh::bounds`, computed in `Mesh::new`; tests with the mesh builders (cuboid, sphere, plane)
   and an empty mesh.
3. `Node::set_culled` / `is_culled`, and `"culled"` in scene files (`scenefile.rs`) and its
   documentation in [../scene-format.md](../scene-format.md). Demo 3 sets it on its waving
   plates.
4. `Culling`, `DrawItem::in_view`, `draw_items(Option<&Culling>)`; tests: a node out of the
   camera's view but in the light's is a caster and not `in_view`; one out of both is not
   listed; `set_culled(false)` keeps it; hidden nodes stay hidden. Existing tests pass `None`.
5. `collect_scene_draws`: build `Culling`, casters from all items, main pass from `in_view`
   items, sort after filtering.
6. `FrameStats`, the `debug` log line, `FeatherApp::on_frame_stats`, and the line in the
   testapp HUD (`testapp_hud.rs`).
6b. Demo 15: split the static meshes per block (`geometry.rs`, `add_static`), one node per
    block and texture; measure with culling off and on.
7. Measure (see *Check*), write the results into the done note when archiving.

## Status (2026-11-14)

Steps 1–6b done. First numbers, Windows, release, from the `debug` log line (per frame, a few
seconds into each demo, camera moving):

| Demo | Main-pass draws | Culled | Shadow draws | Collect time |
|---|---|---|---|---|
| 15 (statics split per block, ~1150 nodes) | 175–210 | 940–975 | 355–395 | 0.35–0.7 ms |
| 16 (~720 nodes) | 110–150 | 570–610 | 190–235 | 0.3–0.4 ms |
| 11 (crawl) | 43 | 0 | 0 | 0.06 ms |

Demo 11 culls nothing: its text is a few meshes, not a node per glyph. Still to do: step 7
(before/after on the S25, the visual checks below).

## Check

- Demo 16 and demo 11 on Windows and the Galaxy S25: draws per frame and `collect_draws` time
  before and after, camera looking along a street (most of the city behind or beside it).
- Demo 15: expected gain is small (only cars, people, lamps); GPU time from the same timestamp
  method as before if it is worth showing.
- Shadows: in demo 15/16, buildings behind the camera still cast their shadows into view
  (screenshots before/after at the same frame, if the demo's time can be fixed).
- Nothing visible pops at the screen edges while the camera turns; text scrolling in and out at
  the edges of the screen (demo 11) appears whole.

## Decisions

All open questions answered (2026-11-14).

1. **Demo 15's merged statics** — *decided: split them.* The static meshes are split per block
   (25 blocks × textures, a few dozen draws instead of a few), so culling shows in demo 15 too.
   This is a step of its own (demo change, after step 5), measured before and after culling.
2. **Opt-out flag** — *decided: `Node::set_culled(false)`.* Demo 3's shader
   (`demo_003_waving_atlas_shader.vert`) bends its flat plates along Z beyond their (flat) box,
   so its plates set it. Every other demo shader only multiplies by `model`, so culling is exact
   for them. A bounds margin (`set_bounds_margin`) can come later if a scene with many displaced
   nodes needs them culled. GuitarPrompter's shaders are to be checked for displacement when
   ported (the C++ engine never culled).
3. **Frame counts** — *decided: committed, logged at `debug`, and shown in the testapp HUD.*
   The app has no handle to the engine's `App`, so the engine hands the counts over:
   `FeatherApp::on_frame_stats(&mut self, stats: &FrameStats)` (default does nothing), called
   once per frame after the draws are collected. `FrameStats` (public): nodes listed, culled by
   the camera, main-pass draws, shadow draws, collect time. The HUD shows a line such as
   `draws 182 · culled 410 · shadow 95 · 0.6 ms`, refreshed with the frame rate; the HUD's own
   nodes are included in the counts (a constant few).
4. **`draw_items` signature** — *decided: `draw_items(Option<&Culling>)`*, one function. The
   engine passes the camera's and light's frustums; tests (in `scene.rs`, `text.rs` and the
   testapp's `testapp.rs`, about 17 calls) pass `None` and get every drawable node, as today.
