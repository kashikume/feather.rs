# Scene file format

A scene file describes a whole `Scene` in JSON:
- the objects it uses: atlases, textures, meshes, shaders and materials;
- the tree of nodes that places those objects in the world.

Load one with:

```rust
let assets = create_assets_provider("assets")?;
let scene = Scene::load(&*assets, "viking_room.scene.json")?;
let room = scene.find_node("Room").unwrap();
```

`Scene::from_json(assets, text)` does the same from JSON text that is already in memory.

The loader is in [`src/feather/scenefile.rs`](../src/feather/scenefile.rs).

## Principles

- **Names, not handles.** Every object in a list has a name, and other objects refer to it by
  that name. Handles are given out when the scene is loaded and never appear in the file.
- **No binary data.** Images, meshes and shaders are separate files. The scene file holds only
  their asset names, which are read through the `AssetsProvider`. On Windows that means paths
  relative to the `assets/` directory next to the executable, e.g. `"viking_room.png"` or
  `"textures/wall.png"` (always with `/`).
- **Strict.** Unknown fields, duplicate names, unknown names and wrong value types are errors.
  The error message says where the problem is, so a typo never goes unnoticed.

## Example

The viking room used by the test app ([`resources/viking_room.scene.json`](../resources/viking_room.scene.json)):

```json
{
  "atlases": [
    { "name": "Room atlas", "file": "viking_room.png" }
  ],
  "textures": [
    { "name": "Room texture", "atlas": "Room atlas" }
  ],
  "meshes": [
    { "name": "Room", "file": "viking_room.obj" }
  ],
  "shaders": [
    {
      "name": "Basic",
      "vertex": "shader.vert.spv",
      "fragment": "shader.frag.spv",
      "model_uniform": "model",
      "view_uniform": "view",
      "projection_uniform": "proj"
    }
  ],
  "materials": [
    {
      "name": "Room",
      "shader": "Basic",
      "uniforms": {
        "texSampler": { "texture": "Room texture" }
      }
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
| `textures`  | array of textures | no       | Rectangles of atlases                      |
| `meshes`    | array of meshes   | no       | Geometry                                   |
| `shaders`   | array of shaders  | no       | Vertex + fragment programs                 |
| `materials` | array of materials| no       | A shader with uniform values               |
| `root`      | node              | **yes**  | The root of the node tree                  |

The lists are loaded in this order:
1. atlases;
2. textures;
3. meshes;
4. shaders;
5. materials;
6. nodes.

So an object can only refer to objects of a kind loaded before it. Within a list, the order of
items doesn't matter.

Names must be unique within each list. The same name may be used in different lists: a mesh and a
material can both be called `"Room"`.

## Atlases

An atlas is an image that one or more textures are cut from.

| Field  | Type   | Required | Description                     |
| ------ | ------ | -------- | ------------------------------- |
| `name` | string | yes      | Unique among atlases            |
| `file` | string | yes      | PNG asset                       |

Any PNG color type and bit depth is accepted; the image is converted to 8-bit RGBA. On the GPU
the atlas becomes one sRGB image with mipmaps.

## Textures

A texture is a rectangle of an atlas.

| Field   | Type          | Required | Description                                          |
| ------- | ------------- | -------- | ---------------------------------------------------- |
| `name`  | string        | yes      | Unique among textures                                |
| `atlas` | string        | yes      | Name of an atlas                                     |
| `rect`  | array of 4 numbers | no  | `[u_min, v_min, u_range, v_range]` in UV space (0–1 across the atlas) |

- **Without `rect`:** the texture covers the whole atlas and becomes the atlas's *full texture*.
- **With `rect`:** the texture covers that part of the atlas. For example, `[0.5, 0, 0.5, 0.5]`
  is the right half of the top half.

Shaders always sample the whole atlas. To use only a texture's rectangle, a shader declares a
`vec4` uniform named after its sampler plus `Rect` (e.g. `texSamplerRect` for `texSampler`), and
maps its coordinates with it:

```glsl
outColor = texture(texSampler, texSamplerRect.xy + uv * texSamplerRect.zw);
```

The engine fills that uniform automatically. Full-atlas textures don't need it.

## Meshes

| Field  | Type   | Required | Description                     |
| ------ | ------ | -------- | ------------------------------- |
| `name` | string | yes      | Unique among meshes             |
| `file` | string | yes      | Wavefront OBJ asset             |

The OBJ file must have positions, texture coordinates and normals. Faces are triangulated, and
OBJ materials (`.mtl`) are ignored.

## Shaders

| Field                | Type   | Required | Description                                         |
| -------------------- | ------ | -------- | --------------------------------------------------- |
| `name`               | string | yes      | Unique among shaders                                |
| `vertex`             | string | yes      | SPIR-V asset of the vertex stage                    |
| `fragment`           | string | yes      | SPIR-V asset of the fragment stage                  |
| `model_uniform`      | string | no       | Uniform that receives the node's model matrix       |
| `view_uniform`       | string | no       | Uniform that receives the camera's view matrix      |
| `projection_uniform` | string | no       | Uniform that receives the camera's projection matrix |

The build compiles GLSL files from `shaders/` into `assets/` as `<file>.spv`, so
`shaders/shader.vert` is referenced as `"shader.vert.spv"`.

The three matrix uniforms are filled by the engine for every node drawn. When given, each name
must exist in the shader as a `mat4`, in a uniform block or in push constants. Leave a field out
if the shader doesn't use that matrix.

Other uniforms (in blocks, push constants and samplers) take their values from materials and
nodes, matched by the GLSL member or sampler name.

## Materials

| Field      | Type   | Required | Description                         |
| ---------- | ------ | -------- | ----------------------------------- |
| `name`     | string | yes      | Unique among materials              |
| `shader`   | string | yes      | Name of a shader                    |
| `uniforms` | object | no       | Uniform values, see [Uniforms](#uniforms) |

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
| `texture` | name of a texture             | `sampler2D` |

Example:

```json
"uniforms": {
  "texSampler": { "texture": "Room texture" },
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

The engine then adds the model, view and projection matrices under the shader's names, and the
`<sampler>Rect` values of the textures. Those always win over values from the file.

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
- a name refers to an object that doesn't exist (e.g. `"mesh": "Rom"`);
- a file is missing or invalid (not a PNG, OBJ or SPIR-V);
- a shader uses resources the engine doesn't support, or its matrix uniform names are wrong;
- a transform has both `matrix` and translation/rotation/scale.

The message lists where the problem is, from the outside in:

```
Cannot load scene viking_room.scene.json: node `Scene root`: node `Room`: unknown mesh `Rom`
```

## Not in the format yet

- **Cameras:** the application creates its own (`FeatherApp::get_camera_to_render_scene`).
- **Other mesh sources:** procedural meshes (e.g. `MeshBuilderCuboid`) can only be created in
  code; the file supports OBJ only.
- **More than one root node.**
