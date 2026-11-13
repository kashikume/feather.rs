# Rendering nodes: open items

Every visible node with a mesh and a material is drawn with its uniforms; how it works is in
[../archive/done-node-uniforms-rendering.md](../archive/done-node-uniforms-rendering.md).

- Sub-textures: the atlas creator's 1-pixel frame stops linear filtering from bleeding at
  full size, but smaller mipmap levels still mix neighbouring textures, and `REPEAT` wraps
  around the whole atlas; add more padding, limit the mip level, or per-texture sampler modes
- More than one texture per material (e.g. a texture per sampler)
- Re-upload an atlas when its pixels change (no API changes pixels yet)
- An atlas removed and another added under the same handle in one frame keeps the old GPU
  image; give objects a generation counter
- Sort opaque draws by material and front to back too; skip redundant descriptor binds
- Sort transparent draws per triangle or with order-independent transparency: sorting by
  node origin is wrong for large or intersecting transparent objects
- Re-upload uniforms only for nodes that changed
- Per-frame data (camera) in its own set, filled once per frame instead of per node
