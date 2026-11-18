# Multiple textures per material: plan

Goal: **a material holds up to 4 textures** (slots 0–3), and each sampler of its shader samples
the slot it is given. First use: a **normal map** next to the colour texture, shown by a new
demo, a rotating tennis ball.

Covers item 13 of [missing-features-vs-cpp.md](missing-features-vs-cpp.md) (multiple textures
per material) and the "More than one texture per material" line of
[scene-format.md](../scene-format.md#not-in-the-format-yet).

## What the C++ engine does

- `Material` keeps `std::vector<Texture::Ptr> m_textures`; material files fill slots with
  `texture <index> atlas <atlas> <texture>` or `texture <index> single <atlas>`, any index
  (the vector grows to fit). No limit.
- `Node` can override the slots (`Node::set_texture(pos, texture)`, copying the material's
  vector first); `get_textures()` returns the node's or else the material's.
- The batcher sorts nodes by the atlases of **all** slots, but **binds only slot 0**:
  `node->getAtlas(0)->use()` on texture unit 0. Unit 1 is the shadow map. Generated shaders
  declare only `texSampler0`, and `Mesh` sets only `texUmin0` / `texUrange0` / `texVmin0` /
  `texVrange0` from `textures[0]`.
- So slots 1+ are stored and sorted on, but never reach a shader. Not worth porting as is.
- GuitarPrompter: every `texture` line in its materials uses index 0 (no multi-texture use).
- No normal mapping anywhere; vertices have no tangents.

## What feather.rs does today

- `Material` has one `Option<Texture>` (`get_texture`, `set_texture`,
  `Scene::material_set_texture`).
- `sampler_sources` (`uniformdata.rs`): every sampler of the shader samples the material's
  texture's atlas, except the one named `shadow_map_sampler`, which samples the shadow map.
- The draw (`app.rs`) already passes a **list** of `(image view, sampler)` per draw, one per
  sampler of the shader, and descriptor sets are cached per list (`pipeline.rs`). So the GPU side
  already handles several different images per draw.
- The texture rectangle uniforms (`umin_uniform`, ...) come from the one texture.
- Every atlas is uploaded as `R8G8B8A8_SRGB` (`gpuimage.rs`): right for colours, **wrong for a
  normal map** (the GPU would decode its values as sRGB, bending the normals).
- `Vertex` is position, normal, UV: no tangent.

## Proposal

### Material: 4 slots

```rust
pub const MAX_MATERIAL_TEXTURES: usize = 4;

pub struct Material {
    textures: [Option<Texture>; MAX_MATERIAL_TEXTURES],
    ...
}

impl Material {
    pub fn get_texture(&self) -> Option<&Texture>;             // slot 0, as today
    pub fn set_texture(&mut self, texture: Option<Texture>);   // slot 0, as today
    pub fn get_texture_at(&self, slot: usize) -> Option<&Texture>;          // None past 3
    pub fn set_texture_at(&mut self, slot: usize, texture: Option<Texture>); // panics past 3
    pub fn get_textures(&self) -> &[Option<Texture>; MAX_MATERIAL_TEXTURES];
}
```

`Scene::material_set_texture_at(material, slot, texture)` checks the material, the atlas and the
slot (error past 3); `material_set_texture` stays as the slot 0 shortcut. Existing code and demos
don't change. A fixed array keeps `Material` small and copyable per slot, no allocation.

### Shader: which sampler samples which slot

The shader names its samplers in order, like it already names `shadow_map_sampler`:

```json
{
  "name": "Tennis ball",
  "vertex": "...", "fragment": "...",
  "texture_samplers": ["colorMap", "normalMap"],
  ...
}
```

- `texture_samplers[i]` samples slot `i`; at most 4 names; each must be a `sampler2D` of the
  shader, not the shadow map sampler (else the shader fails to load, like a wrong matrix name).
- **`texture_samplers` is required** for every shader with samplers other than the shadow
  map: a sampler listed nowhere (and not the shadow map) is an error at load, so there is one
  rule and no implicit slot 0. A shader without such samplers (the shadow map only, or none)
  doesn't need the field. Every existing shader with a texture is migrated (see Steps).
- A sampler whose slot is empty in the material samples the fallback texture (magenta) and logs
  one warning, as a missing texture does today (`UniformIssue::NoTexture` gets the slot).

Alternative considered: slots by binding order. Rejected: invisible in the shader, and breaks
when someone adds a sampler in between.

`sampler_sources` takes `&[Option<Texture>; 4]` and the shader's slot list, and returns
`SamplerSource::Atlas(slot atlas)` per sampler. `app.rs` changes only in what it passes.

### Texture rectangles for every slot

A `texture_samplers` entry is either a sampler name or an object that also names the slot's
rectangle uniforms:

```json
"texture_samplers": [
  { "sampler": "colorMap", "umin": "colorUmin", "umax": "colorUmax",
    "vmin": "colorVmin", "vmax": "colorVmax" },
  "normalMap"
]
```

- The engine fills each named rectangle from that slot's texture, like the shader-level
  `umin_uniform` ... `vmax_uniform` today. A plain name means the slot needs no rectangle
  (whole image).
- The shader-level `umin_uniform` ... `vmax_uniform` are **removed** (no backward
  compatibility): a shader that uses a rectangle lists its sampler in `texture_samplers` with
  the rectangle. An old field in a scene file is a load error (unknown field) pointing to
  `texture_samplers`.
- `ShaderUniformNames` loses `umin` ... `vmax` and `with_texture_rect`; it gets
  `texture_samplers: Vec<TextureSamplerNames>` (sampler name + optional rectangle names) and a
  builder `with_texture_sampler(name, rect)`. `ShaderUniformIds` gets a rectangle per slot
  (`[Option<RectIds>; 4]`); `engine_uniforms` takes the material's slots and emits each slot's
  four floats. The names are checked at load like the existing ones (`float`, present in the
  shader).
- Migrated in the same change: the scene files of demos 2, 3, 8, 9, 16, 19 and 20 (and
  `tools/generate_demo_016_city_assets.py`, which writes demo 16's), the text shaders' names in
  `text.rs` (demos 10 and 21 and the testapp HUD go through it), the tests in `shader.rs`,
  `uniformdata.rs`, `scenefile.rs`, and the README and scene-format.md examples. The GLSL
  shaders don't change: only where their uniform names are given.

### Atlas colour space: `linear` atlases

A normal map stores vectors, not colours, so it must be sampled without sRGB decoding.

- Atlas in the scene file: `"color_space": "linear"` (default `"srgb"`).
- `Atlas` gets `ColorSpace { Srgb, Linear }` and `get_color_space`. Every constructor takes it
  as a parameter, so it is visible at each call:
  `Atlas::new_from_png(assets, bitmap, metadata, name, color_space)`,
  `new_from_png_bytes(png, name, color_space)`,
  `from_rgba(name, width, height, pixels, color_space)` and
  `Scene::create_atlas_from_png(assets, bitmap, metadata, name, color_space)`. No setter: the
  colour space is fixed when the atlas is made (it decides the GPU image format).
- Callers migrated: the scene loader (from `color_space`), `text.rs` (font atlases, sRGB),
  demo 15, the feather-ui tests, the engine's own tests (`atlas.rs`, `scene.rs`) and the README
  examples.
- `GpuImage` uploads `Linear` atlases as `R8G8B8A8_UNORM`. Mipmaps by blitting work the same
  (UNORM supports linear blits on every Vulkan device, Android included). Averaged mip levels
  shorten the normals; the shader normalizes after sampling.
- The atlas creator (`.atlist`) is not involved: normal maps are single images.

### Tangents: computed in the fragment shader, no vertex change

Normal maps are in tangent space, which needs a tangent and bitangent per pixel. Two ways:

1. **From screen-space derivatives in the fragment shader** (`dFdx` / `dFdy` of the world
   position and the UV build the tangent frame, "cotangent frame" method). No engine change,
   works with every mesh source (OBJ, fmesh, sphere, cuboid, plane). Costs a few instructions
   per pixel; slightly faceted on very low-poly meshes.
2. **A tangent attribute in `Vertex`**: changes the vertex layout of every mesh, the fmesh
   format, the OBJ/fmesh converter and every mesh builder, and every vertex shader's inputs.

**Proposal: 1.** It is shader code only, in the demo's shader (and documented in
scene-format.md as the recommended way). Tangents in vertices can come later as their own plan if
a mesh needs them.

### Scene file: `textures`

A material gets either `texture` (as today, slot 0) or `textures`, an array of up to 4 entries,
each like `texture` or `null` for an empty slot:

```json
{
  "name": "Tennis ball",
  "shader": "Tennis ball",
  "textures": [
    { "atlas": "Tennis ball felt" },
    { "atlas": "Tennis ball normals" }
  ],
  "uniforms": { "normalStrength": { "float": 1.0 } }
}
```

Load errors: both `texture` and `textures`; more than 4 entries; an unknown atlas or texture
name (as today).

## Demo 023: tennis ball

A tennis ball turning slowly about a tilted axis under a fixed light, close to the camera so the
felt and the seam fill the screen.

- **Mesh:** the generated sphere, `"sphere": { "radius": 1.0, "segments": 128, "rings": 64 }`
  (equirectangular UVs, see scene-format.md).
- **Slot 0, `tennis_ball.png`** (sRGB, 2048 × 1024, equirectangular): optic yellow-green felt
  (around `#D4E62A`) with fine fibre noise and soft colour blotches, and the white seam: two
  closed curves meeting in the classic tennis ball shape, drawn as a band about 2 % of the ball's
  circumference wide.
- **Slot 1, `tennis_ball_normal.png`** (linear, same size): tangent-space normals (OpenGL
  convention, green = up in the image) from a height field: the seam as a groove with a rounded
  profile, the felt as fine fuzzy bumps. Generated from the same seam curve, so the two images
  line up.
- Both images made by **`tools/scripts/make_tennis_ball_textures.py`** (Python, standard
  library only, writes the PNGs with `zlib`, like `make_feather_obj.py` needs nothing
  installed), committed to `apps/testapp/resources/`. The seam curve is the usual
  parametrization `p(t) = (a·cos t + b·cos 3t, a·sin t − b·sin 3t, c·sin 2t)` normalized onto
  the sphere; for each pixel the script finds the angular distance to the curve.
- **Shader `demo_023_tennis_ball_shader.{vert,frag}`:** passes world position, normal and UV;
  the fragment shader samples `colorMap`, samples `normalMap`, builds the tangent frame from
  derivatives, scales the perturbation by `normalStrength`, and lights with diffuse plus a weak,
  wide specular (felt is matte) and a little wrap lighting for the fuzzy look.
- **No interaction:** the light comes from the side (grazing on part of the ball), so as the
  ball turns the seam groove and the fuzz catch and lose the light; that shows the relief.
- Turning: like the globe demo (a node rotated each frame in the demo's `update`), or a scene
  animation if one fits (the animation demos have them).
- Works on Windows and Android (no extensions: derivatives and UNORM images are core Vulkan).

## Steps

1. `Material`: 4 slots, `get/set_texture_at`, `get_textures`; `Scene::material_set_texture_at`;
   tests (slot 0 shortcuts unchanged, slot bounds, invalid atlas).
2. Shader: `texture_samplers` (names or objects with rectangle uniforms) in
   `ShaderUniformNames` / the shader JSON, checked against the shader's samplers and float
   uniforms at load (tests for each error); `engine_uniforms` fills every slot's rectangle;
   shader-level `umin_uniform` ... `vmax_uniform` removed and every user migrated (see "Texture
   rectangles for every slot").
3. `sampler_sources` per slot; `app.rs` passes the material's slots; tests (unlisted sampler =
   load error, two slots, empty slot = fallback + issue, shadow map unaffected). Every shader
   with a texture sampler gets `texture_samplers`: the shaders in the testapp scene files (and
   the demo 16 generator script), shaders created in code (`text.rs`, demos built in code,
   testapp HUD) and the doc examples. The tutorial's feather shader has no sampler, so it
   doesn't change.
4. Atlas `ColorSpace` as a constructor parameter (callers migrated), scene file
   `color_space`, `GpuImage` format per atlas; tests for parsing and the format choice.
5. Scene file `textures` array (with `null`), errors; tests.
6. `make_tennis_ball_textures.py`, the two PNGs; demo 023 scene, shaders, demo code, toggle;
   testapp scene-load test like the other demos'.
7. Docs: scene-format.md (atlas `color_space`, shader `texture_samplers`, material `textures`,
   normal mapping with derivative tangents and a GLSL snippet; drop the "not in the format yet"
   line), missing-features item 13 (done except per-node textures, item 5), README demo list.
8. Run on Windows and on an Android phone; check the seam lines up across the `u` = 0/1 edge.

## Not in this plan

- Per-node texture overrides (item 5 of missing-features-vs-cpp.md).
- Tangents in vertices / the fmesh format.
- More than 4 slots (`MAX_MATERIAL_TEXTURES` can be raised later; only the API shows it),
  texture arrays, cube maps.

## Decisions

1. **Limit 4**, as the constant `MAX_MATERIAL_TEXTURES`.
2. **Every slot can have its own rectangle uniforms** (see "Texture rectangles for every slot").
3. **Colour space from code:** a `ColorSpace` parameter on every atlas constructor and
   `Scene::create_atlas_from_png`; no setter.
4. **Demo:** one ball, no on/off switch; the lighting shows the relief.
