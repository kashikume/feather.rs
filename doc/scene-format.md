# Scene file format

A scene file describes a whole `Scene` in JSON:
- the objects it uses: atlases, meshes, shaders and materials;
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
    { "name": "Room", "file": "demo_000_viking_room_mesh.obj" }
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
| `root`      | node              | **yes**  | The root of the node tree                  |

The lists are loaded in this order:
1. atlases;
2. meshes;
3. shaders;
4. materials;
5. nodes.

So an object can only refer to objects of a kind loaded before it. Within a list, the order of
items doesn't matter.

Names must be unique within each list. The same name may be used in different lists: a mesh and a
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

A mesh comes either from an OBJ file or is a cuboid or a plane generated by the engine. Give
exactly one of `file`, `cuboid` and `plane`.

| Field    | Type   | Required | Description                     |
| -------- | ------ | -------- | ------------------------------- |
| `name`   | string | yes      | Unique among meshes             |
| `file`   | string | one of   | Wavefront OBJ asset             |
| `cuboid` | object | one of   | A box, see below                |
| `plane`  | object | one of   | A flat grid, see below          |

The OBJ file must have positions, texture coordinates and normals. Faces are triangulated, and
OBJ materials (`.mtl`) are ignored.

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

The test app's build compiles GLSL files from `apps/testapp/shaders/` into `assets/` as `<file>.spv`, so
`shaders/demo_000_viking_room_shader.vert` is referenced as
`"demo_000_viking_room_shader.vert.spv"`.

The three matrix uniforms and the four texture edges are filled by the engine for every node
drawn. When given, each name must exist in the shader, in a uniform block or in push constants:
the matrices as `mat4`, the edges as `float`. Leave a field out if the shader doesn't use that
value. The edges come from the node's material's texture (see [Textures](#textures)).

Other uniforms (in blocks and push constants) take their values from materials and nodes, matched
by the GLSL member name. Samplers sample the atlas of the material's texture.

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
- a transform has both `matrix` and translation/rotation/scale.

The message lists where the problem is, from the outside in:

```
Cannot load scene demo_000_viking_room_scene.json: node `Scene root`: node `Room`: unknown mesh `Rom`
```

## Not in the format yet

- **Cameras:** the application creates its own (`FeatherApp::get_camera_to_render_scene`).
- **Other mesh sources:** only OBJ files, cuboids and planes; other generated meshes must be
  created in code.
- **More than one texture per material.**
- **More than one root node.**
