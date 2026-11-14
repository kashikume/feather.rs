# Scene file format

A scene file describes a whole `Scene` in JSON:
- the objects it uses: atlases, meshes, shaders and materials;
- the cameras it can be seen through;
- the tree of nodes that places those objects in the world.

Load one with:

```rust
let assets = create_assets_provider("assets")?;
let scene = Scene::load(&*assets, "demo_000_viking_room_scene.json")?;
let room = scene.find_node("Room").unwrap();
```

`Scene::from_json(assets, text)` does the same from JSON text that is already in memory.

The loader is in [`crates/feather/src/scenefile.rs`](../crates/feather/src/scenefile.rs).

## Principles

- **Names, not handles.** Every object in a list has a name, and other objects refer to it by
  that name. Handles are given out when the scene is loaded and never appear in the file.
- **No binary data.** Images, atlas metadata, meshes and shaders are separate files. The scene file holds only
  their asset names, which are read through the `AssetsProvider`. On Windows that means paths
  relative to the `assets/` directory next to the executable, e.g.
  `"demo_000_viking_room_atlas.png"` or `"textures/wall.png"` (always with `/`).
- **Strict.** Unknown fields, duplicate names, unknown names and wrong value types are errors.
  The error message says where the problem is, so a typo never goes unnoticed.

## Example

The viking room used by the test app ([`apps/testapp/resources/demo_000_viking_room_scene.json`](../apps/testapp/resources/demo_000_viking_room_scene.json)):

```json
{
  "atlases": [
    { "name": "Room atlas", "bitmap": "demo_000_viking_room_atlas.png" }
  ],
  "meshes": [
    { "name": "Room", "file": "demo_000_viking_room_mesh.fmesh" }
  ],
  "shaders": [
    {
      "name": "Basic",
      "vertex": "demo_000_viking_room_shader.vert.spv",
      "fragment": "demo_000_viking_room_shader.frag.spv",
      "model_uniform": "model",
      "view_uniform": "view",
      "projection_uniform": "proj"
    }
  ],
  "materials": [
    {
      "name": "Room",
      "shader": "Basic",
      "texture": { "atlas": "Room atlas" }
    }
  ],
  "cameras": [
    {
      "name": "Camera",
      "perspective": { "fov": 45 },
      "eye": [2, 2, 2],
      "target": [0, 0, 0],
      "up": [0, 0, 1],
      "near": 0.1,
      "far": 10
    }
  ],
  "root": {
    "name": "Scene root",
    "children": [
      { "name": "Room", "mesh": "Room", "material": "Room" }
    ]
  }
}
```

## Top level

| Field       | Type              | Required | Description                                |
| ----------- | ----------------- | -------- | ------------------------------------------ |
| `atlases`   | array of atlases  | no       | Images that textures are cut from          |
| `meshes`    | array of meshes   | no       | Geometry                                   |
| `shaders`   | array of shaders  | no       | Vertex + fragment programs                 |
| `materials` | array of materials| no       | A shader with uniform values               |
| `shadow_light` | object         | no       | The light casting the scene's shadows, see [Shadows](#shadows) |
| `cameras`   | array of cameras  | no       | What the scene can be seen through, see [Cameras](#cameras) |
| `active_camera` | string        | no       | Name of the camera the scene is drawn through (default: the first) |
| `root`      | node              | **yes**  | The root of the node tree                  |

The lists are loaded in this order:
1. atlases;
2. meshes;
3. shaders;
4. materials;
5. cameras;
6. nodes.

So an object can only refer to objects of a kind loaded before it. Within a list, the order of
items doesn't matter.

Names must be unique within each list (cameras don't need one, but named cameras must have
different names). The same name may be used in different lists: a mesh and a
material can both be called `"Room"`.

## Atlases

An atlas is an image that textures are cut from, with optional metadata naming its textures.

| Field      | Type   | Required | Description                                 |
| ---------- | ------ | -------- | ------------------------------------------- |
| `name`     | string | yes      | Unique among atlases                        |
| `bitmap`   | string | yes      | PNG asset                                   |
| `metadata` | string | no       | `.atlas` asset naming rectangles of the PNG |

Any PNG color type and bit depth is accepted; the image is converted to 8-bit RGBA. On the GPU
the atlas becomes one sRGB image with mipmaps.

The metadata lists one texture per line, `name x y width height`, in pixels from the top-left
corner of the image (the name may contain spaces; empty lines are skipped):

```
plate_01 1 1 128 128
plate_02 1 131 128 128
```

The atlas creator (`tools/atlas-creator`) writes such a file next to the atlas it packs; the test
app builds its atlases this way from `.atlist` lists (see the README). Without `metadata` only
the whole atlas can be used as a texture.

### Textures

Textures are not listed in the file: a material names the atlas and, optionally, the texture
(see [Materials](#materials)). A texture is the rectangle of the atlas that the metadata gives
for its name, or the whole atlas when no name is given.

Shaders always sample the whole atlas. To use only a texture's rectangle, a shader declares
`float` uniforms for its edges, names them in its `umin_uniform`, `umax_uniform`,
`vmin_uniform` and `vmax_uniform` (see [Shaders](#shaders)), and maps its coordinates with them:

```glsl
layout(binding = 2) uniform TextureRect { float umin; float umax; float vmin; float vmax; } rect;
...
outColor = texture(atlas, mix(vec2(rect.umin, rect.vmin), vec2(rect.umax, rect.vmax), uv));
```

The engine fills them from the material's texture (UV space, 0–1 across the atlas, `v` = 0 at
the top row). Whole-atlas textures don't need them.

## Meshes

A mesh comes either from a file or is a cuboid, a plane or a sphere generated by the engine.
Give exactly one of `file`, `cuboid`, `plane` and `sphere`.

| Field    | Type   | Required | Description                     |
| -------- | ------ | -------- | ------------------------------- |
| `name`   | string | yes      | Unique among meshes             |
| `file`   | string | one of   | `.fmesh` or `.obj` asset        |
| `cuboid` | object | one of   | A box, see below                |
| `plane`  | object | one of   | A flat grid, see below          |
| `sphere` | object | one of   | A ball, see below               |

The file's extension picks how it's read:

- **`.fmesh`**: feather's binary mesh ([fmesh-format.md](fmesh-format.md)), made from an OBJ by
  the `mesh-converter` tool. The test app's `build.rs` converts every OBJ of `resources/` into
  an `.fmesh` asset, so its scene files name `.fmesh` files.
- **`.obj`**: a Wavefront OBJ, read when the scene loads. It must have positions, texture
  coordinates and normals. Faces are triangulated, and OBJ materials (`.mtl`) are ignored.

Both give the same mesh; the `.fmesh` loads about ten times faster and is much smaller.

### Cuboids

A box with walls parallel to the axes. All fields are optional: `"cuboid": {}` is a 1×1×1 cube
centred on the origin (`MeshBuilderCuboid::default()`).

| Field | Type                       | Default       | Description                                 |
| ----- | -------------------------- | ------------- | ------------------------------------------- |
| `x`   | `[min, max]`               | `[-0.5, 0.5]` | Extent along X                              |
| `y`   | `[min, max]`               | `[-0.5, 0.5]` | Extent along Y                              |
| `z`   | `[min, max]`               | `[-0.5, 0.5]` | Extent along Z                              |
| `u`   | array of `[min, max]`      | `[[0, 1]]`    | Horizontal texture coordinates of each wall |
| `v`   | array of `[min, max]`      | `[[0, 1]]`    | Vertical texture coordinates of each wall   |

- Without `u` and `v`, every wall shows the whole texture.
- The lists are used wall by wall, in the order: Z min, X max, Z max, X min, Y min, Y max. A
  list shorter than 6 repeats, so `[[0, 0.5], [0.5, 1]]` alternates between the two halves.

A unit cube centred on the origin, and a 1×1×2 column standing on the XY plane:

```json
{ "name": "Cube", "cuboid": {} },
{ "name": "Column", "cuboid": { "z": [0, 2] } }
```

To get other sizes, a unit cube can also be scaled by its node's `transform`, as the cuboid demo
does.

### Planes

A flat rectangle in the XY plane (z = 0) facing +Z, split into a grid so that vertex shaders can
bend it (`MeshBuilderPlane`). All fields are optional: `"plane": {}` is a 1×1 square centred on
the origin, as one cell.

| Field      | Type               | Default       | Description                  |
| ---------- | ------------------ | ------------- | ---------------------------- |
| `x`        | `[min, max]`       | `[-0.5, 0.5]` | Extent along X               |
| `y`        | `[min, max]`       | `[-0.5, 0.5]` | Extent along Y               |
| `segments` | `[columns, rows]`  | `[1, 1]`      | Number of cells, at least 1  |

The whole texture covers the plane as seen from +Z: `u` goes from 0 at `x` min to 1 at `x` max,
`v` from 0 at the top (`y` max) to 1 at the bottom. The plates of the waving atlas demo hang from their
top edge:

```json
{ "name": "Plate", "plane": { "x": [-0.5, 0.5], "y": [-1, 0], "segments": [12, 12] } }
```

### Spheres

A ball centred on the origin, with its poles on the Z axis (`MeshBuilderSphere`). All fields
are optional: `"sphere": {}` is a ball of diameter 1, like the default cuboid.

| Field      | Type   | Default | Description                                      |
| ---------- | ------ | ------- | ------------------------------------------------ |
| `radius`   | number | `0.5`   | Radius, greater than 0                           |
| `segments` | number | `48`    | Slices around the Z axis, at least 3             |
| `rings`    | number | `24`    | Bands from pole to pole, at least 2              |

The texture wraps around the ball like a world map in equirectangular projection: `u` goes from
0 to 1 eastwards starting at -X (so +X is the middle of the image), `v` from 0 at the north pole
(+Z) to 1 at the south pole. More segments and rings make a smoother ball. The globe demo:

```json
{ "name": "Ball", "sphere": { "radius": 1.0, "segments": 96, "rings": 48 } }
```

## Shaders

| Field                | Type   | Required | Description                                         |
| -------------------- | ------ | -------- | --------------------------------------------------- |
| `name`               | string | yes      | Unique among shaders                                |
| `vertex`             | string | yes      | SPIR-V asset of the vertex stage                    |
| `fragment`           | string | yes      | SPIR-V asset of the fragment stage                  |
| `model_uniform`      | string | no       | Uniform that receives the node's model matrix       |
| `view_uniform`       | string | no       | Uniform that receives the camera's view matrix      |
| `projection_uniform` | string | no       | Uniform that receives the camera's projection matrix |
| `umin_uniform`       | string | no       | Uniform that receives the left edge of the material's texture |
| `umax_uniform`       | string | no       | Uniform that receives the right edge of the material's texture |
| `vmin_uniform`       | string | no       | Uniform that receives the top edge of the material's texture |
| `vmax_uniform`       | string | no       | Uniform that receives the bottom edge of the material's texture |
| `shadow_matrix_uniform` | string | no    | Uniform that receives the shadow light's view-projection matrix (`mat4`), see [Shadows](#shadows) |
| `shadow_map_sampler` | string | no       | Sampler that samples the shadow map instead of the material's texture |

The test app's build compiles GLSL files from `apps/testapp/shaders/` into `assets/` as `<file>.spv`, so
`shaders/demo_000_viking_room_shader.vert` is referenced as
`"demo_000_viking_room_shader.vert.spv"`.

The three matrix uniforms and the four texture edges are filled by the engine for every node
drawn. When given, each name must exist in the shader, in a uniform block or in push constants:
the matrices as `mat4`, the edges as `float`. Leave a field out if the shader doesn't use that
value. The edges come from the node's material's texture (see [Textures](#textures)).

Other uniforms (in blocks and push constants) take their values from materials and nodes, matched
by the GLSL member name. Samplers sample the atlas of the material's texture, except the
`shadow_map_sampler`, which samples the scene's shadow map.

## Materials

| Field      | Type   | Required | Description                         |
| ---------- | ------ | -------- | ----------------------------------- |
| `name`     | string | yes      | Unique among materials              |
| `shader`   | string | yes      | Name of a shader                    |
| `texture`  | object | no       | The texture its shader samples, see below |
| `uniforms` | object | no       | Uniform values, see [Uniforms](#uniforms) |

`texture` names an atlas and, optionally, a texture from the atlas's metadata:

| Field   | Type   | Required | Description                                              |
| ------- | ------ | -------- | -------------------------------------------------------- |
| `atlas` | string | yes      | Name of an atlas                                         |
| `name`  | string | no       | Texture name from the atlas's metadata; without it, the whole atlas |

```json
{ "name": "Room", "shader": "Basic", "texture": { "atlas": "Room atlas" } },
{ "name": "Plate 1", "shader": "Waving", "texture": { "atlas": "Plates", "name": "plate_01" } }
```

Every sampler of the shader samples the texture's atlas. A shader with samplers used by a
material without a texture draws magenta, and a warning is logged.

## Nodes

`root` is a node; every node can have `children`.

| Field       | Type             | Required | Description                                    |
| ----------- | ---------------- | -------- | ---------------------------------------------- |
| `name`      | string           | no       | Used by `Scene::find_node` and in errors       |
| `mesh`      | string           | no       | Name of a mesh                                 |
| `material`  | string           | no       | Name of a material                             |
| `transform` | object           | no       | Position relative to the parent, see [Transforms](#transforms) |
| `visible`   | boolean          | no       | `false` hides the node and its descendants (default `true`) |
| `transparent` | boolean        | no       | `true` blends the node over what is behind it, using its shader's output alpha; drawn after opaque nodes, back to front (default `false`, not inherited) |
| `casts_shadow` | boolean       | no       | `true` makes the node's mesh cast a shadow when the scene has a `shadow_light` (default `false`, not inherited) |
| `culled`    | boolean          | no       | `false` draws the node even when its mesh is outside the camera's (or the shadow light's) view; for shaders that move vertices beyond the mesh (default `true`, not inherited) |
| `uniforms`  | object           | no       | Uniform values, see [Uniforms](#uniforms)      |
| `children`  | array of nodes   | no       | Child nodes                                    |

- A node is drawn when it is visible, its ancestors are visible, and it has both a mesh and a
  material.
- Nodes without a mesh are useful for grouping: their transform and uniforms apply to their
  children.
- Node names don't have to be unique. `Scene::find_node` returns the first node with the name,
  so give unique names to the nodes the application looks up.

## Uniforms

`uniforms` is an object that maps uniform names to values. Each value is an object with one
field, named after the GLSL type:

| Key       | JSON value                    | GLSL type   |
| --------- | ----------------------------- | ----------- |
| `float`   | number                        | `float`     |
| `int`     | integer                       | `int`       |
| `uint`    | non-negative integer          | `uint`      |
| `bool`    | `true` / `false`              | `bool`      |
| `vec2`    | array of 2 numbers            | `vec2`      |
| `vec3`    | array of 3 numbers            | `vec3`      |
| `vec4`    | array of 4 numbers            | `vec4`      |
| `ivec2`, `ivec3`, `ivec4` | array of 2, 3, 4 integers | `ivec2`, `ivec3`, `ivec4` |
| `uvec2`, `uvec3`, `uvec4` | array of 2, 3, 4 non-negative integers | `uvec2`, `uvec3`, `uvec4` |
| `mat2`    | array of 4 numbers, column-major  | `mat2`  |
| `mat3`    | array of 9 numbers, column-major  | `mat3`  |
| `mat4`    | array of 16 numbers, column-major | `mat4`  |

Example:

```json
"uniforms": {
  "tint":       { "vec4": [1.0, 0.8, 0.8, 1.0] },
  "shininess":  { "float": 32 },
  "lit":        { "bool": true }
}
```

Column-major means the numbers are listed column by column. A translation by (5, 6, 7) is:

```json
{ "mat4": [1, 0, 0, 0,  0, 1, 0, 0,  0, 0, 1, 0,  5, 6, 7, 1] }
```

### Which value a node's shader gets

Each node's uniforms are combined in three layers. A later layer overrides an earlier one with
the same name:

1. the uniforms of the node's material;
2. the uniforms inherited from its ancestor nodes (their own uniforms, not their materials');
3. the node's own uniforms.

The engine then adds the model, view and projection matrices and the edges of the material's
texture under the shader's names. Those always win over values from the file.

The file isn't checked against the shaders. Problems that depend on the shader are logged once
as warnings while drawing, not reported when loading:
- a shader uniform that has no value;
- a value whose type doesn't match the shader;
- a value the shader doesn't use.

## Transforms

A transform positions a node relative to its parent. It is either a matrix:

| Field    | Type                | Description                     |
| -------- | ------------------- | ------------------------------- |
| `matrix` | array of 16 numbers | Column-major 4×4 matrix         |

or any combination of:

| Field         | Type                         | Default     | Description                        |
| ------------- | ---------------------------- | ----------- | ---------------------------------- |
| `translation` | array of 3 numbers           | `[0, 0, 0]` | Offset                             |
| `rotation`    | array of 3 numbers           | `[0, 0, 0]` | Degrees around X, then Y, then Z   |
| `scale`       | number or array of 3 numbers | `1`         | Uniform or per-axis scale          |

These are applied scale first, then rotation, then translation, i.e. `T × Rz × Ry × Rx × S`.
`matrix` can't be combined with the other fields. Without `transform`, a node has the identity
transform.

```json
"transform": { "translation": [0, -1.2, 0.3], "rotation": [0, 0, 45], "scale": 0.5 }
```

The engine uses a right-handed coordinate system. The viking room's camera has Z pointing up.

## Errors

Loading fails, and nothing of the scene is kept, when:
- the JSON is invalid, or a field is unknown or has the wrong type;
- two items in the same list have the same name;
- a name refers to an object that doesn't exist (e.g. `"mesh": "Rom"`), or a material names a
  texture its atlas's metadata doesn't have;
- a file is missing or invalid (not a PNG, `.atlas`, OBJ or SPIR-V), or a metadata rectangle is
  outside its atlas;
- a shader uses resources the engine doesn't support, or its matrix or texture edge uniform
  names are wrong;
- a transform has both `matrix` and translation/rotation/scale;
- a camera has both or neither of `perspective` and `orthographic`, or invalid settings (see
  [Cameras](#cameras)), or `active_camera` names no camera.

The message lists where the problem is, from the outside in:

```
Cannot load scene demo_000_viking_room_scene.json: node `Scene root`: node `Room`: unknown mesh `Rom`
```

## Shadows

`shadow_light` is a directional light, like the sun, casting the scene's shadows:

```json
"shadow_light": { "direction": [-1, 0.6, -1.2], "center": [0, 0, 1], "radius": 10.5, "map_size": 2048 }
```

| Field       | Type      | Required | Description                                          |
| ----------- | --------- | -------- | ---------------------------------------------------- |
| `direction` | [x, y, z] | yes      | The direction the light's rays go (need not be normalized) |
| `center`    | [x, y, z] | no       | The middle of the area that gets shadows (default `[0, 0, 0]`) |
| `radius`    | number    | yes      | The area's radius: shadows are cast within it (above 0) |
| `map_size`  | integer   | no       | Width and height of the shadow map in pixels (default 2048) |

Before each frame, the engine draws the nodes with `"casts_shadow": true` from the light into a
shadow map. Shaders receive shadows by naming a `shadow_matrix_uniform` (a `mat4`, the light's
view-projection matrix) and a `shadow_map_sampler`, and comparing depths (see the README's
[Shadows](../README.md#shadows)). A smaller radius gives sharper shadows in a smaller area.

## Cameras

A scene is drawn through its active camera. `cameras` lists the scene's cameras, which are added
in that order; the first one is the active camera, unless `active_camera` names another. A
scene without cameras isn't drawn until the application adds one (`Scene::add_camera`).
Applications find cameras with `Scene::find_camera`, move them, and switch the active one with
`Scene::set_active_camera`.

```json
"cameras": [
  { "name": "Street", "perspective": { "fov": 60 },
    "eye": [0, -75, 4.5], "target": [18, -75, 3.3], "up": [0, 0, 1], "near": 0.2, "far": 520 },
  { "name": "Overlay", "orthographic": { "origin": "top_left", "height": 768 } }
],
"active_camera": "Street"
```

| Field          | Type      | Required | Description                                            |
| -------------- | --------- | -------- | ------------------------------------------------------ |
| `name`         | string    | no       | Used by `Scene::find_camera`, `active_camera` and in errors |
| `perspective`  | object    | one of   | A perspective projection, see below                    |
| `orthographic` | object    | one of   | A flat projection, for 2D scenes, see below            |
| `eye`          | [x, y, z] | no       | Where the camera is                                    |
| `target`       | [x, y, z] | no       | The point it looks at (not `eye`)                      |
| `up`           | [x, y, z] | no       | Which way is up on the screen (not zero)               |
| `near`, `far`  | number    | no       | The visible depth range along the view direction (`near` below `far`) |

`perspective`:

| Field | Type   | Default | Description                                   |
| ----- | ------ | ------- | --------------------------------------------- |
| `fov` | number | `45`    | Vertical field of view in degrees, 0 to 180   |

Defaults: `eye` `[2, 2, 2]`, `target` `[0, 0, 0]`, `up` `[0, 1, 0]`, `near` `0.1`, `far` `10`.
`near` must be above 0.

`orthographic`:

| Field    | Type   | Default    | Description                                            |
| -------- | ------ | ---------- | ------------------------------------------------------ |
| `origin` | string | `"center"` | Where (0, 0) is on the screen: `"top_left"`, `"center"` or `"bottom_left"` |
| `height` | number | none       | The screen's height in scene units, above 0; the width follows the window's aspect ratio. Without it, one unit per pixel |

Defaults: `eye` `[0, 0, 0]`, `target` `[0, 0, -1]` and `up` `[0, 1, 0]`, so it shows the XY
plane with Y up; `near` `-1000` and `far` `1000`, so objects between z = -1000 and z = 1000 are
visible, the nearer ones (greater z) over the farther ones. With `"origin": "top_left"` and
`"height": 768`, the screen spans x `0..width` and y `-768..0`.

## Not in the format yet

- **Moving lights:** `shadow_light` is fixed; applications move it with `Scene::set_shadow_light`.
- **Other mesh sources:** only OBJ files, cuboids, planes and spheres; other generated meshes
  must be created in code.
- **More than one texture per material.**
- **More than one root node.**
