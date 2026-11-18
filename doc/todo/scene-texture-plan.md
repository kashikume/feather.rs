# A scene drawn into a texture: plan

Goal: **a scene can be drawn into a texture that another scene samples**, every frame. First
use: a new demo, a **drive-in cinema**: the window shows a night scene with a big screen and two
parked cars, and the screen shows another scene, demo 0's turning viking room, live.

File references are relative to `crates/feather/src/`.

## What the C++ engine does

Nothing to port. Its only framebuffer object is the shadow map's (`feather-RenderingState.cpp`:
`glGenFramebuffers`, a colour texture with packed depth); scenes are always drawn to the window.
GuitarPrompter doesn't render to textures either. This is new work.

## What feather.rs has today

- **Several scenes per frame** (`FeatherApp::get_num_scenes_to_render`, `get_scene_to_render`):
  all drawn into the window's render pass, one over the other, each through its active camera.
  The engine keeps the GPU state of each scene by its id (`SceneGpuState` in `AppData::scenes`),
  and releases the state of scenes no longer rendered (`release_unrendered_scenes`).
- **One offscreen pass**: the shadow map (`shadowpass.rs`): its own render pass and image,
  recorded before the main pass in the same command buffer, then sampled by shaders through
  `SamplerSource::ShadowMap`. The pattern to follow.
- **Samplers** take an atlas (`SamplerSource::Atlas`) or the shadow map; descriptor sets are
  cached per image view, so a new kind of image costs nothing there.
- **Pipelines** are per scene (`SceneGpuState::shader_pipelines`) but all built for one target:
  the window's render pass, MSAA sample count and extent (`PipelineTarget`; the viewport is fixed
  in the pipeline).
- **The camera's aspect** comes from the window: `collect_scene_draws` calls
  `camera.set_screen_dimention(extent)` with the swapchain's extent.

## What is missing

1. **A texture whose pixels come from a scene.** Atlases are uploaded once from pixels. The outer
   scene needs an atlas that is a GPU image written every frame by drawing another scene, with
   a size but no pixels, that materials use like any other (`Texture::full(atlas)`).
2. **A way to name which scene draws into it.** Scenes belong to the app, not to the engine, and
   don't know each other. The engine must learn which scenes to draw offscreen, and into which
   atlas.
3. **An offscreen render pass and its images**: a colour image the size of the texture (sRGB,
   like atlases, so sampling gives linear colours), a depth image, and with MSAA a multisampled
   colour image resolved into the texture. Then a barrier so the outer scene's fragment shaders
   can read it.
4. **Pipelines for that target.** A scene drawn offscreen needs its pipelines built against the
   offscreen render pass, its sample count and its extent, not the window's. Since pipelines are
   per scene, each scene's state gets its own `PipelineTarget`.
5. **The camera's aspect from the target**, not the window: the inner camera's projection uses
   the texture's width and height.
6. **Frame order**: offscreen scenes (and their shadow passes) are collected and recorded before
   the window's scenes, in the same command buffer.
7. **Keeping their GPU state**: offscreen scenes count as rendered, or `release_unrendered_scenes`
   destroys their state every frame.
8. **Mipmaps** (optional): a screen seen from afar shimmers without them. They can be blitted
   from the new image every frame (as atlases are at upload), at a small cost.

Nothing is missing for the rest of the demo: cars can reuse the city's vehicle OBJs
(`city_sedan.obj`, `city_hatchback.obj` and their texture), the room comes from demo 0's scene
file, and the screen is a plane with an unlit shader showing its texture.

## Proposal

### Scene file and API: a "scene texture" atlas

An atlas is either a PNG (as today) or a **scene texture**: a size, no pixels.

```json
"atlases": [
  { "name": "Screen", "scene_texture": { "width": 1280, "height": 544 } }
]
```

```rust
let screen = scene.add_atlas(Atlas::scene_texture(Some("Screen".into()), 1280, 544));
```

Materials use it like any atlas. Until a scene draws into it, it samples as the fallback
texture (or black).

### FeatherApp: scenes drawn into textures

```rust
/// Scenes drawn into scene textures before the window's scenes, every frame.
fn get_num_texture_scenes(&self) -> usize { 0 }
/// Texture scene `index` and where it is drawn: the id of the scene that has the scene texture
/// (one of the window's scenes) and the atlas's handle there.
fn get_texture_scene(&mut self, index: usize) -> (&mut Scene, SceneTextureTarget);
```

Default: none, so existing apps don't change. The test app's `Demo` trait gets the same pair
of methods (default: none), and `TestApp` passes them on.

The engine, each frame:
1. For each texture scene: collect its draws as for a window scene, with the camera's aspect
   from the target atlas's size, into its target's offscreen pass.
2. Record each offscreen pass (shadow pass first, if it has one), then a barrier.
3. The window's scenes: a sampler whose atlas is a scene texture binds that offscreen image.

The offscreen images live in the **target** scene's GPU state, by atlas handle (like
`gpu_atlases`), so they go away with it. A scene texture no texture scene draws into, or whose
target names a missing atlas, is logged once.

### Rendering details

- **Colour format**: `R8G8B8A8_SRGB`, as atlases, sampled with the shared texture sampler.
- **MSAA**: the device's sample count by default, resolved into the texture (as the window
  does); `"msaa": false` in the scene texture draws it with one sample. The demo is measured
  both ways on the S25 to choose the default (open question 2).
- **Mipmaps**: generated every frame by blitting (decided), so the texture is filtered like an
  atlas at any distance.
- **Image orientation**: Vulkan's framebuffer row 0 is the top, and atlases have `v` = 0 at the
  top row, so the texture maps the same way as an image would. To check in the demo.
- **Synchronization**: one image per scene texture. A barrier at the start of each offscreen pass
  (fragment shader reads before → colour writes) covers the previous frame still sampling it.
- **Stats**: texture scenes get a `SceneStats` each, so the HUD shows them.

## Demo: drive-in cinema

- **Inner scene**: demo 0's scene file (`demo_000_viking_room_scene.json`), its room turning as in
  demo 0, its camera's aspect set by the screen's size (1280 × 544, about 2.35:1, a cinema
  screen).
- **Outer scene** (`demo_023_drive_in_scene.json`): night. A wide screen on two posts, a ground
  plane with parking rows, two cars (the city's sedan and hatchback) facing the screen, a dark
  sky. The screen's material: an unlit shader showing the scene texture, slightly brighter than
  1 so it glows. The cars and the ground lit by a dim moon light, plus a cool light from the
  screen's direction.
- **Camera**: behind the cars, slowly swaying, so the screen is seen at an angle and from afar
  (where mipmaps matter).
- Optional: the screen casts its average colour onto the cars (read back is too slow; the demo
  could use a fixed, flickering value).

## Steps

1. `Atlas::scene_texture` and `"scene_texture"` in scene files; tests.
2. `FeatherApp::get_num_texture_scenes` / `get_texture_scene`, `SceneTextureTarget`; the test
   app's `Demo` and `TestApp`.
3. Offscreen render pass and images (`scenetexture.rs`, like `shadowpass.rs`); a
   `PipelineTarget` per scene's GPU state; the camera's aspect from the target.
4. Collecting and recording texture scenes before the window's; `SamplerSource::SceneTexture`;
   keeping their GPU state; stats.
5. Mipmaps every frame.
6. The demo: scene file, screen shader, demo code, test.
7. Docs: README (features, demo table, a "Scenes in textures" section), scene-format.md, and the
   Android lifecycle notes if it changes anything there.
8. Run on Windows and an Android phone.

## Decisions

1. **API shape**: the app lists its texture scenes with their targets
   (`FeatherApp::get_num_texture_scenes` / `get_texture_scene`, see above). Scenes don't store
   a target; the link is made in code (one line in the demo).
2. **MSAA**: on by default (the device's sample count), `"msaa": false` in the scene texture
   turns it off; the demo is measured both ways on the S25 to keep or change the default.
3. **Mipmaps**: generated every frame.
4. **Nesting**: none. A texture scene can't show a scene texture itself; if it has one, it
   samples the fallback and a warning is logged once.
5. **Size**: fixed in the atlas (`width`, `height`).
