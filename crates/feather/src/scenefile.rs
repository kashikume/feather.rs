//! Loading a scene from a JSON file.
//!
//! The file lists the scene's atlases, meshes, shaders, materials and cameras, and describes the
//! node tree starting from the root. Objects refer to each other by name; files (PNG, `.atlas`, OBJ,
//! SPIR-V) are asset names read through an `AssetsProvider`. Example:
//!
//! ```json
//! {
//!   "atlases":  [{ "name": "Room", "bitmap": "demo_000_viking_room_atlas.png" },
//!                { "name": "Icons", "bitmap": "icons.png", "metadata": "icons.atlas" }],
//!   "meshes":   [{ "name": "Room", "file": "demo_000_viking_room_mesh.fmesh" },
//!                { "name": "Box", "cuboid": { "x": [-1, 1], "y": [-1, 1], "z": [0, 2] } },
//!                { "name": "Flag", "plane": { "x": [0, 2], "y": [-1, 0], "segments": [8, 4] } },
//!                { "name": "Ball", "sphere": { "radius": 1, "segments": 64, "rings": 32 } }],
//!   "shaders":  [{ "name": "Basic",
//!                  "vertex": "demo_000_viking_room_shader.vert.spv",
//!                  "fragment": "demo_000_viking_room_shader.frag.spv",
//!                  "model_uniform": "model", "view_uniform": "view",
//!                  "projection_uniform": "proj",
//!                  "umin_uniform": "umin", "umax_uniform": "umax",
//!                  "vmin_uniform": "vmin", "vmax_uniform": "vmax",
//!                  "shadow_matrix_uniform": "lightViewProj",
//!                  "shadow_map_sampler": "shadowMap" }],
//!   "materials": [{ "name": "Room", "shader": "Basic", "texture": { "atlas": "Room" } },
//!                 { "name": "Star", "shader": "Basic",
//!                   "texture": { "atlas": "Icons", "name": "star" } }],
//!   "shadow_light": { "direction": [1, 0.5, -2], "center": [0, 0, 0], "radius": 6 },
//!   "cameras": [{ "name": "Main", "perspective": { "fov": 45 },
//!                 "eye": [2, 2, 2], "target": [0, 0, 0.3], "up": [0, 0, 1], "far": 10 },
//!               { "name": "Flat", "orthographic": { "origin": "center", "height": 10 } }],
//!   "active_camera": "Main",
//!   "root": {
//!     "name": "Scene root",
//!     "children": [{
//!       "name": "Room", "mesh": "Room", "material": "Room",
//!       "transform": { "translation": [0, 0, 0], "rotation": [0, 0, 90], "scale": 1.0 },
//!       "uniforms": { "tint": { "vec4": [1, 0.5, 0.5, 1] } }
//!     }]
//!   }
//! }
//! ```
//!
//! - Every list item needs a name, unique within its list (cameras: if they have one). All
//!   lists are optional.
//! - An atlas is a PNG `bitmap` with optional `metadata` (a `.atlas` file naming rectangles of
//!   the bitmap, see `atlas::parse_metadata`).
//! - A material's `texture` is a texture of an atlas: the rectangle `name` from the atlas's
//!   metadata, or the whole atlas when `name` is missing.
//! - A mesh is a `file`, an `.fmesh` (`MeshBuilderFmeshFile`, made from an OBJ by the
//!   `mesh-converter` tool) or a Wavefront `.obj` (`MeshBuilderObjFile`), chosen by the
//!   extension; or a `cuboid` with optional `x`, `y`, `z` ranges (default
//!   `[-0.5, 0.5]`, so `"cuboid": {}` is a 1x1x1 cube centred on the origin) and optional
//!   per-wall `u`, `v` texture ranges, a `plane` (`MeshBuilderPlane`) with optional `x`,
//!   `y` ranges (default `[-0.5, 0.5]`) and `segments` (default `[1, 1]`), or a `sphere`
//!   (`MeshBuilderSphere`) with optional `radius` (default 0.5), `segments` (default 48) and
//!   `rings` (default 24).
//! - Uniform values are written as `{ "<glsl type>": value }`: `float`, `int`, `uint`, `bool`,
//!   `vec2`-`vec4`, `ivec2`-`ivec4`, `uvec2`-`uvec4` (arrays), or `mat2`-`mat4` (column-major
//!   arrays of 4, 9 or 16 numbers).
//! - A node transform is either `matrix` (16 numbers, column-major) or any of `translation`
//!   `[x, y, z]`, `rotation` `[x, y, z]` in degrees (applied around X, then Y, then Z) and
//!   `scale` (a number or `[x, y, z]`). Nodes may also set `"visible": false`,
//!   `"transparent": true` (see `Node::set_transparent`), `"casts_shadow": true` and
//!   `"culled": false` (see `Node::set_culled`).
//! - `shadow_light` (optional) is a directional light casting the scene's shadows
//!   (`ShadowLight::directional`): its `direction`, the `center` (default the origin) and
//!   `radius` of the area it covers, and an optional `map_size` in pixels (default 2048).
//! - A camera (`Camera`) has either `perspective` (with an optional `fov` in degrees, default
//!   45) or `orthographic` (with an optional `origin`, `"top_left"`, `"center"` (default) or
//!   `"bottom_left"`, and an optional `height` in scene units; without it, one unit per pixel).
//!   Both take optional `eye`, `target`, `up`, `near` and `far`, with the defaults of
//!   `Camera::perspective` and `Camera::orthographic`. Cameras are added in order, so the
//!   first one is the active camera unless `active_camera` names another.
//! - Unknown fields are errors, so typos don't go unnoticed.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{anyhow, bail, Context, Result};
use cgmath::{Deg, Matrix2, Matrix3, Matrix4};
use serde::Deserialize;

use super::assetsprovider::AssetsProvider;
use super::camera::{Camera, ScreenOrigin, ScreenUnits};
use super::math::{IVec2, IVec3, IVec4, Mat4, Point3, UVec2, UVec3, UVec4, Vec2, Vec3, Vec4};
use super::meshbuildercuboid::MeshBuilderCuboid;
use super::meshbuilderfmeshfile::MeshBuilderFmeshFile;
use super::meshbuilderobjfile::MeshBuilderObjFile;
use super::meshbuilderplane::MeshBuilderPlane;
use super::meshbuildersphere::{self, MeshBuilderSphere};
use super::scene::Scene;
use super::shader::ShaderUniformNames;
use super::shadow::ShadowLight;
use super::uniform::UniformValue;
use super::uniforms::Uniforms;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneFile {
    #[serde(default)]
    atlases: Vec<AtlasEntry>,
    #[serde(default)]
    meshes: Vec<MeshEntry>,
    #[serde(default)]
    shaders: Vec<ShaderEntry>,
    #[serde(default)]
    materials: Vec<MaterialEntry>,
    shadow_light: Option<ShadowLightEntry>,
    #[serde(default)]
    cameras: Vec<CameraEntry>,
    /// Name of the camera the scene is drawn through; the first camera by default.
    active_camera: Option<String>,
    root: NodeEntry,
}

/// A camera (`Camera`): `perspective` or `orthographic`, plus the settings both have.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CameraEntry {
    name: Option<String>,
    perspective: Option<PerspectiveEntry>,
    orthographic: Option<OrthographicEntry>,
    eye: Option<[f32; 3]>,
    target: Option<[f32; 3]>,
    up: Option<[f32; 3]>,
    near: Option<f32>,
    far: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PerspectiveEntry {
    /// Vertical field of view in degrees.
    fov: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OrthographicEntry {
    #[serde(default)]
    origin: OriginEntry,
    /// Scene units shown vertically; without it, one unit per pixel.
    height: Option<f32>,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum OriginEntry {
    TopLeft,
    #[default]
    Center,
    BottomLeft,
}

/// A directional light casting the scene's shadows (`ShadowLight::directional`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShadowLightEntry {
    direction: [f32; 3],
    #[serde(default)]
    center: [f32; 3],
    radius: f32,
    map_size: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AtlasEntry {
    name: String,
    /// PNG asset.
    bitmap: String,
    /// `.atlas` asset naming rectangles of the bitmap.
    metadata: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeshEntry {
    name: String,
    /// An `.fmesh` or `.obj` asset. Exactly one of `file`, `cuboid`, `plane` and `sphere` must
    /// be given.
    file: Option<String>,
    cuboid: Option<CuboidEntry>,
    plane: Option<PlaneEntry>,
    sphere: Option<SphereEntry>,
}

/// A box with walls parallel to the axes (`MeshBuilderCuboid`). `{}` is the default 1x1x1 cube
/// centred on the origin.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CuboidEntry {
    /// `[min, max]` along each axis; `[-0.5, 0.5]` when missing.
    x: Option<[f32; 2]>,
    y: Option<[f32; 2]>,
    z: Option<[f32; 2]>,
    /// Texture coordinates `[min, max]` of each wall, in wall order; when the list is shorter
    /// than 6 it repeats. Default: the whole texture on every wall.
    #[serde(default)]
    u: Vec<[f32; 2]>,
    #[serde(default)]
    v: Vec<[f32; 2]>,
}

/// A flat rectangle in the XY plane split into a grid (`MeshBuilderPlane`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlaneEntry {
    /// `[min, max]` along each axis; `[-0.5, 0.5]` when missing.
    x: Option<[f32; 2]>,
    y: Option<[f32; 2]>,
    /// `[columns, rows]`; `[1, 1]` when missing.
    segments: Option<[u32; 2]>,
}

/// A ball centred on the origin with its poles on the Z axis (`MeshBuilderSphere`). `{}` is the
/// default ball of diameter 1.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SphereEntry {
    radius: Option<f32>,
    /// Slices around the Z axis.
    segments: Option<u32>,
    /// Bands from pole to pole.
    rings: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShaderEntry {
    name: String,
    /// SPIR-V asset of the vertex stage.
    vertex: String,
    /// SPIR-V asset of the fragment stage.
    fragment: String,
    model_uniform: Option<String>,
    view_uniform: Option<String>,
    projection_uniform: Option<String>,
    umin_uniform: Option<String>,
    umax_uniform: Option<String>,
    vmin_uniform: Option<String>,
    vmax_uniform: Option<String>,
    shadow_matrix_uniform: Option<String>,
    shadow_map_sampler: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialEntry {
    name: String,
    shader: String,
    texture: Option<TextureEntry>,
    #[serde(default)]
    uniforms: BTreeMap<String, UniformEntry>,
}

/// A texture of an atlas: the rectangle `name` from the atlas's metadata, or the whole atlas.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextureEntry {
    atlas: String,
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeEntry {
    name: Option<String>,
    mesh: Option<String>,
    material: Option<String>,
    transform: Option<TransformEntry>,
    #[serde(default = "default_visible")]
    visible: bool,
    #[serde(default)]
    transparent: bool,
    #[serde(default)]
    casts_shadow: bool,
    #[serde(default = "default_culled")]
    culled: bool,
    #[serde(default)]
    uniforms: BTreeMap<String, UniformEntry>,
    #[serde(default)]
    children: Vec<NodeEntry>,
}

fn default_visible() -> bool {
    true
}

fn default_culled() -> bool {
    true
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct TransformEntry {
    /// Column-major; can't be combined with the other fields.
    matrix: Option<[f32; 16]>,
    translation: Option<[f32; 3]>,
    /// Degrees around X, then Y, then Z.
    rotation: Option<[f32; 3]>,
    scale: Option<ScaleEntry>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ScaleEntry {
    Uniform(f32),
    PerAxis([f32; 3]),
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
enum UniformEntry {
    Float(f32),
    Int(i32),
    Uint(u32),
    Bool(bool),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Ivec2([i32; 2]),
    Ivec3([i32; 3]),
    Ivec4([i32; 4]),
    Uvec2([u32; 2]),
    Uvec3([u32; 3]),
    Uvec4([u32; 4]),
    Mat2([f32; 4]),
    Mat3([f32; 9]),
    Mat4([f32; 16]),
}

impl Scene {
    /// Loads a scene from the JSON asset `asset`; see the `scenefile` module for the format.
    /// Files named in the scene are read through `assets` too.
    pub fn load(assets: &dyn AssetsProvider, asset: &str) -> Result<Scene> {
        let json = assets.get_asset(asset)?;
        let json = std::str::from_utf8(&json).with_context(|| format!("{asset} is not UTF-8"))?;
        // Windows tools often start UTF-8 files with a byte order mark.
        let json = json.strip_prefix('\u{feff}').unwrap_or(json);
        Self::from_json(assets, json).with_context(|| format!("Cannot load scene {asset}"))
    }

    /// Builds a scene from JSON text; see the `scenefile` module for the format.
    pub fn from_json(assets: &dyn AssetsProvider, json: &str) -> Result<Scene> {
        let file: SceneFile = serde_json::from_str(json)?;
        let mut scene = Scene::new();

        check_unique("atlas", file.atlases.iter().map(|a| &a.name))?;
        check_unique("mesh", file.meshes.iter().map(|m| &m.name))?;
        check_unique("shader", file.shaders.iter().map(|s| &s.name))?;
        check_unique("material", file.materials.iter().map(|m| &m.name))?;
        check_unique(
            "camera",
            file.cameras.iter().filter_map(|c| c.name.as_ref()),
        )?;

        let mut atlases = HashMap::new();
        for entry in &file.atlases {
            let handle = scene
                .create_atlas_from_png(
                    assets,
                    &entry.bitmap,
                    entry.metadata.as_deref(),
                    Some(entry.name.clone()),
                )
                .with_context(|| format!("atlas `{}`", entry.name))?;
            atlases.insert(entry.name.as_str(), handle);
        }

        let mut meshes = HashMap::new();
        let start = std::time::Instant::now();
        for entry in &file.meshes {
            let handle = build_mesh(&mut scene, assets, entry)
                .with_context(|| format!("mesh `{}`", entry.name))?;
            meshes.insert(entry.name.as_str(), handle);
        }
        if !file.meshes.is_empty() {
            log::info!(
                "{} meshes built in {:.1} ms",
                file.meshes.len(),
                start.elapsed().as_secs_f64() * 1000.0
            );
        }

        let mut shaders = HashMap::new();
        for entry in &file.shaders {
            let handle = (|| {
                let names = ShaderUniformNames {
                    model: entry.model_uniform.clone(),
                    view: entry.view_uniform.clone(),
                    projection: entry.projection_uniform.clone(),
                    umin: entry.umin_uniform.clone(),
                    umax: entry.umax_uniform.clone(),
                    vmin: entry.vmin_uniform.clone(),
                    vmax: entry.vmax_uniform.clone(),
                    shadow_matrix: entry.shadow_matrix_uniform.clone(),
                    shadow_map: entry.shadow_map_sampler.clone(),
                };
                scene.create_shader(
                    Some(entry.name.clone()),
                    &assets.get_asset(&entry.vertex)?,
                    &assets.get_asset(&entry.fragment)?,
                    names,
                )
            })()
            .with_context(|| format!("shader `{}`", entry.name))?;
            shaders.insert(entry.name.as_str(), handle);
        }

        let mut materials = HashMap::new();
        for entry in &file.materials {
            let handle = (|| {
                let shader = *shaders
                    .get(entry.shader.as_str())
                    .ok_or_else(|| anyhow!("unknown shader `{}`", entry.shader))?;
                let material = scene.create_material(Some(entry.name.clone()), shader)?;
                if let Some(texture) = &entry.texture {
                    let texture = find_texture(&scene, &atlases, texture)?;
                    scene.material_set_texture(material, Some(texture))?;
                }
                let uniforms = scene.get_material_mut(material).unwrap().uniforms_mut();
                set_uniforms(uniforms, &entry.uniforms)?;
                Ok::<_, anyhow::Error>(material)
            })()
            .with_context(|| format!("material `{}`", entry.name))?;
            materials.insert(entry.name.as_str(), handle);
        }

        let names = Names { meshes, materials };
        if let Some(light) = &file.shadow_light {
            if !(light.radius > 0.0) {
                bail!("shadow_light: radius must be above 0, got {}", light.radius);
            }
            let mut shadow = ShadowLight::directional(
                Vec3::from(light.direction),
                Point3::from(light.center),
                light.radius,
            );
            if let Some(size) = light.map_size {
                shadow = shadow.with_map_size(size);
            }
            scene.set_shadow_light(Some(shadow));
        }

        for (index, entry) in file.cameras.iter().enumerate() {
            let camera = build_camera(entry).with_context(|| match &entry.name {
                Some(name) => format!("camera `{name}`"),
                None => format!("camera {index}"),
            })?;
            scene.add_camera(camera);
        }
        if let Some(name) = &file.active_camera {
            let camera = scene
                .find_camera(name)
                .ok_or_else(|| anyhow!("active_camera: unknown camera `{name}`"))?;
            scene.set_active_camera(camera)?;
        }

        let root = scene.create_root_node(file.root.name.clone());
        setup_node(&mut scene, root, &file.root, &names)?;
        Ok(scene)
    }

    /// Handle of the first node named `name`.
    pub fn find_node(&self, name: &str) -> Option<usize> {
        use super::object::Object;
        self.nodes.get_by_name(name).map(|n| n.get_handle())
    }
}

fn build_mesh(scene: &mut Scene, assets: &dyn AssetsProvider, entry: &MeshEntry) -> Result<usize> {
    let name = Some(entry.name.clone());
    let range = |r: Option<[f32; 2]>| {
        let [min, max] = r.unwrap_or([-0.5, 0.5]);
        (min, max)
    };
    match (&entry.file, &entry.cuboid, &entry.plane, &entry.sphere) {
        (Some(file), None, None, None) => {
            let extension = std::path::Path::new(file)
                .extension()
                .and_then(|e| e.to_str());
            match extension.map(str::to_ascii_lowercase).as_deref() {
                Some("fmesh") => MeshBuilderFmeshFile::new(assets, file)?.build(scene, name),
                Some("obj") => MeshBuilderObjFile::new(assets, file)?.build(scene, name),
                _ => bail!(
                    "mesh {}: unknown file type {file} (.fmesh or .obj expected)",
                    entry.name
                ),
            }
        }
        (None, Some(c), None, None) => {
            let is_default =
                c.x.is_none() && c.y.is_none() && c.z.is_none() && c.u.is_empty() && c.v.is_empty();
            let builder = if is_default {
                MeshBuilderCuboid::default()
            } else {
                let pairs = |list: &[[f32; 2]]| list.iter().map(|[a, b]| (*a, *b)).collect();
                MeshBuilderCuboid::new(
                    range(c.x),
                    range(c.y),
                    range(c.z),
                    Some(pairs(&c.u)),
                    Some(pairs(&c.v)),
                )
            };
            builder.build(scene, name)
        }
        (None, None, Some(p), None) => {
            let [columns, rows] = p.segments.unwrap_or([1, 1]);
            MeshBuilderPlane::new(range(p.x), range(p.y), (columns, rows)).build(scene, name)
        }
        (None, None, None, Some(sphere)) => MeshBuilderSphere::new(
            sphere.radius.unwrap_or(meshbuildersphere::DEFAULT_RADIUS),
            sphere
                .segments
                .unwrap_or(meshbuildersphere::DEFAULT_SEGMENTS),
            sphere.rings.unwrap_or(meshbuildersphere::DEFAULT_RINGS),
        )
        .build(scene, name),
        _ => bail!("a mesh needs exactly one of `file`, `cuboid`, `plane` and `sphere`"),
    }
}

fn build_camera(entry: &CameraEntry) -> Result<Camera> {
    let mut camera = match (&entry.perspective, &entry.orthographic) {
        (Some(perspective), None) => {
            let mut camera = Camera::perspective();
            if let Some(fov) = perspective.fov {
                if !(fov > 0.0 && fov < 180.0) {
                    bail!("fov must be between 0 and 180 degrees, got {fov}");
                }
                camera.set_fov(fov);
            }
            camera
        }
        (None, Some(orthographic)) => {
            let origin = match orthographic.origin {
                OriginEntry::TopLeft => ScreenOrigin::TopLeft,
                OriginEntry::Center => ScreenOrigin::Center,
                OriginEntry::BottomLeft => ScreenOrigin::BottomLeft,
            };
            let units = match orthographic.height {
                None => ScreenUnits::Pixels,
                Some(height) if height > 0.0 => ScreenUnits::Height(height),
                Some(height) => bail!("height must be above 0, got {height}"),
            };
            Camera::orthographic(origin, units)
        }
        _ => bail!("a camera needs exactly one of `perspective` and `orthographic`"),
    };
    if let Some(name) = &entry.name {
        camera = camera.with_name(name.clone());
    }

    let near = entry.near.unwrap_or(camera.get_near());
    let far = entry.far.unwrap_or(camera.get_far());
    if !(near < far) {
        bail!("near ({near}) must be below far ({far})");
    }
    if entry.perspective.is_some() && !(near > 0.0) {
        bail!("near must be above 0 for a perspective camera, got {near}");
    }
    camera.set_near_far(near, far);

    let eye = entry.eye.map_or(camera.get_eye_position(), Point3::from);
    let target = entry
        .target
        .map_or(camera.get_target_position(), Point3::from);
    let up = entry.up.map_or(camera.get_up_vector(), Vec3::from);
    if eye == target {
        bail!("eye and target must differ, both are {:?}", eye);
    }
    if up == Vec3::new(0.0, 0.0, 0.0) {
        bail!("up must not be zero");
    }
    camera.set_view(eye, target, up);
    Ok(camera)
}

/// The texture `entry` refers to, from an atlas loaded earlier.
fn find_texture(
    scene: &Scene,
    atlases: &HashMap<&str, usize>,
    entry: &TextureEntry,
) -> Result<super::texture::Texture> {
    let atlas = *atlases
        .get(entry.atlas.as_str())
        .ok_or_else(|| anyhow!("unknown atlas `{}`", entry.atlas))?;
    let atlas = scene.get_atlas(atlas).unwrap();
    match &entry.name {
        None => Ok(atlas.get_full_texture()),
        Some(name) => atlas
            .get_texture(name)
            .ok_or_else(|| anyhow!("atlas `{}` has no texture `{name}`", entry.atlas)),
    }
}

/// Handles of the scene's named objects that nodes refer to.
struct Names<'a> {
    meshes: HashMap<&'a str, usize>,
    materials: HashMap<&'a str, usize>,
}

/// Applies `entry` to the existing node `handle` and creates its children, recursively.
fn setup_node(scene: &mut Scene, handle: usize, entry: &NodeEntry, names: &Names) -> Result<()> {
    let what = || match &entry.name {
        Some(name) => format!("node `{name}`"),
        None => "unnamed node".to_string(),
    };
    (|| {
        if let Some(mesh) = &entry.mesh {
            let mesh = *names
                .meshes
                .get(mesh.as_str())
                .ok_or_else(|| anyhow!("unknown mesh `{mesh}`"))?;
            scene.node_set_mesh(handle, mesh)?;
        }
        if let Some(material) = &entry.material {
            let material = *names
                .materials
                .get(material.as_str())
                .ok_or_else(|| anyhow!("unknown material `{material}`"))?;
            scene.node_set_material(handle, material)?;
        }
        let node = scene.get_node_mut(handle).unwrap();
        if let Some(transform) = &entry.transform {
            node.set_transform(transform.to_matrix()?);
        }
        node.set_visible(entry.visible);
        node.set_transparent(entry.transparent);
        node.set_casts_shadow(entry.casts_shadow);
        node.set_culled(entry.culled);
        set_uniforms(node.uniforms_mut(), &entry.uniforms)
    })()
    .with_context(what)?;

    for child in &entry.children {
        let child_handle = scene
            .create_node(child.name.clone(), handle)
            .with_context(what)?;
        setup_node(scene, child_handle, child, names).with_context(what)?;
    }
    Ok(())
}

fn set_uniforms(uniforms: &mut Uniforms, entries: &BTreeMap<String, UniformEntry>) -> Result<()> {
    for (name, entry) in entries {
        uniforms.set(name.clone(), entry.to_value());
    }
    Ok(())
}

fn check_unique<'a>(kind: &str, names: impl Iterator<Item = &'a String>) -> Result<()> {
    let mut seen = HashSet::new();
    for name in names {
        if !seen.insert(name) {
            bail!("there is more than one {kind} named `{name}`");
        }
    }
    Ok(())
}

impl UniformEntry {
    fn to_value(&self) -> UniformValue {
        use UniformEntry as E;
        match *self {
            E::Float(v) => UniformValue::Float(v),
            E::Int(v) => UniformValue::Int(v),
            E::Uint(v) => UniformValue::UInt(v),
            E::Bool(v) => UniformValue::Bool(v),
            E::Vec2(v) => UniformValue::Vec2(Vec2::from(v)),
            E::Vec3(v) => UniformValue::Vec3(Vec3::from(v)),
            E::Vec4(v) => UniformValue::Vec4(Vec4::from(v)),
            E::Ivec2(v) => UniformValue::IVec2(IVec2::from(v)),
            E::Ivec3(v) => UniformValue::IVec3(IVec3::from(v)),
            E::Ivec4(v) => UniformValue::IVec4(IVec4::from(v)),
            E::Uvec2(v) => UniformValue::UVec2(UVec2::from(v)),
            E::Uvec3(v) => UniformValue::UVec3(UVec3::from(v)),
            E::Uvec4(v) => UniformValue::UVec4(UVec4::from(v)),
            E::Mat2(m) => UniformValue::Mat2(Matrix2::from([[m[0], m[1]], [m[2], m[3]]])),
            E::Mat3(m) => UniformValue::Mat3(Matrix3::from([
                [m[0], m[1], m[2]],
                [m[3], m[4], m[5]],
                [m[6], m[7], m[8]],
            ])),
            E::Mat4(m) => UniformValue::Mat4(mat4_from_columns(&m)),
        }
    }
}

fn mat4_from_columns(m: &[f32; 16]) -> Mat4 {
    Matrix4::from([
        [m[0], m[1], m[2], m[3]],
        [m[4], m[5], m[6], m[7]],
        [m[8], m[9], m[10], m[11]],
        [m[12], m[13], m[14], m[15]],
    ])
}

impl TransformEntry {
    /// translation × rotation (Z × Y × X) × scale, or `matrix`.
    fn to_matrix(&self) -> Result<Mat4> {
        if let Some(matrix) = &self.matrix {
            if self.translation.is_some() || self.rotation.is_some() || self.scale.is_some() {
                bail!("transform `matrix` can't be combined with translation, rotation or scale");
            }
            return Ok(mat4_from_columns(matrix));
        }
        let translation = Mat4::from_translation(Vec3::from(self.translation.unwrap_or_default()));
        let [x, y, z] = self.rotation.unwrap_or_default();
        let rotation =
            Mat4::from_angle_z(Deg(z)) * Mat4::from_angle_y(Deg(y)) * Mat4::from_angle_x(Deg(x));
        let scale = match self.scale {
            None => Mat4::from_scale(1.0),
            Some(ScaleEntry::Uniform(s)) => Mat4::from_scale(s),
            Some(ScaleEntry::PerAxis([x, y, z])) => Mat4::from_nonuniform_scale(x, y, z),
        };
        Ok(translation * rotation * scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assetsprovider::tests::MemoryAssetsProvider;
    use crate::object::Object;
    use crate::shader::tests::{FRAG, VERT};
    use crate::texture::Texture;
    use cgmath::{vec4, InnerSpace, SquareMatrix};

    const TRIANGLE: &str =
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nvn 0 0 1\nf 1/1/1 2/2/1 3/3/1\n";

    /// A 4x2 PNG, with metadata naming its left and right halves.
    fn png_4x2() -> Vec<u8> {
        let mut png = Vec::new();
        let mut encoder = png::Encoder::new(&mut png, 4, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[255; 32]).unwrap();
        writer.finish().unwrap();
        png
    }

    fn assets() -> MemoryAssetsProvider {
        let mut assets = MemoryAssetsProvider::default();
        for (name, bytes) in [
            ("atlas.png", png_4x2()),
            ("atlas.atlas", b"left 0 0 2 2\nright 2 0 2 2\n".to_vec()),
            ("triangle.obj", TRIANGLE.as_bytes().to_vec()),
            (
                "triangle.fmesh",
                include_bytes!("../testdata/triangle.fmesh").to_vec(),
            ),
            ("basic.vert.spv", VERT.to_vec()),
            ("basic.frag.spv", FRAG.to_vec()),
        ] {
            assets.assets.insert(name.to_string(), bytes);
        }
        assets
    }

    const SCENE: &str = r#"{
        "atlases": [
            { "name": "Atlas", "bitmap": "atlas.png", "metadata": "atlas.atlas" },
            { "name": "Plain", "bitmap": "atlas.png" }
        ],
        "meshes": [{ "name": "Triangle", "file": "triangle.obj" }],
        "shaders": [{
            "name": "Basic", "vertex": "basic.vert.spv", "fragment": "basic.frag.spv",
            "model_uniform": "model", "view_uniform": "view", "projection_uniform": "proj"
        }],
        "materials": [
            {
                "name": "Mat", "shader": "Basic", "texture": { "atlas": "Atlas", "name": "right" },
                "uniforms": { "tint": { "vec4": [1, 0.5, 0.25, 1] } }
            },
            { "name": "Full", "shader": "Basic", "texture": { "atlas": "Plain" } },
            { "name": "Untextured", "shader": "Basic" }
        ],
        "root": {
            "name": "Root",
            "children": [
                {
                    "name": "A", "mesh": "Triangle", "material": "Mat",
                    "transform": { "translation": [1, 2, 3], "scale": 2 },
                    "uniforms": { "count": { "uint": 3 } },
                    "children": [{ "name": "A1", "visible": false }]
                },
                { "name": "B", "transform": { "rotation": [0, 0, 90] }, "transparent": true }
            ]
        }
    }"#;

    #[test]
    fn loads_every_kind_of_object() {
        let scene = Scene::from_json(&assets(), SCENE).unwrap();

        let atlas = scene.atlases.get_by_name("Atlas").unwrap();
        assert_eq!((atlas.get_width(), atlas.get_height()), (4, 2));
        assert_eq!(atlas.get_texture_names(), ["left", "right"]);
        let plain = scene.atlases.get_by_name("Plain").unwrap();
        assert!(plain.get_texture_names().is_empty());

        let mesh = scene.meshes.get_by_name("Triangle").unwrap();
        assert_eq!(mesh.gen_num_indexes(), 3);

        let shader = scene.shaders.get_by_name("Basic").unwrap();
        assert_eq!(shader.get_uniform_proj_name(), Some("proj"));
        assert_eq!(shader.get_uniform_umin_name(), None);

        let material = scene.materials.get_by_name("Mat").unwrap();
        assert_eq!(material.get_shader(), shader.get_handle());
        assert_eq!(
            material.get_texture(),
            Some(&Texture::new(atlas.get_handle(), 0.5, 1.0, 0.0, 1.0))
        );
        assert_eq!(
            material.uniforms().get("tint"),
            Some(&UniformValue::Vec4(vec4(1.0, 0.5, 0.25, 1.0)))
        );
        assert_eq!(
            scene.materials.get_by_name("Full").unwrap().get_texture(),
            Some(&Texture::full(plain.get_handle()))
        );
        assert_eq!(
            scene
                .materials
                .get_by_name("Untextured")
                .unwrap()
                .get_texture(),
            None
        );
    }

    #[test]
    fn builds_the_node_tree() {
        let scene = Scene::from_json(&assets(), SCENE).unwrap();

        let root = scene.find_node("Root").unwrap();
        let a = scene.find_node("A").unwrap();
        let a1 = scene.find_node("A1").unwrap();
        let b = scene.find_node("B").unwrap();
        let root_node = scene.get_node(root).unwrap();
        assert!(root_node.is_root());
        assert_eq!(root_node.get_children().len(), 2);
        assert_eq!(scene.get_node(a1).unwrap().get_parent(), Some(a));
        assert!(!scene.get_node(a1).unwrap().is_visible());
        assert!(scene.get_node(b).unwrap().is_transparent());
        assert!(!scene.get_node(a).unwrap().is_transparent());

        let node_a = scene.get_node(a).unwrap();
        assert_eq!(
            node_a.get_mesh(),
            Some(scene.meshes.get_by_name("Triangle").unwrap().get_handle())
        );
        assert_eq!(
            node_a.get_material(),
            Some(scene.materials.get_by_name("Mat").unwrap().get_handle())
        );
        assert_eq!(
            node_a.get_transform(),
            Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)) * Mat4::from_scale(2.0)
        );
        assert_eq!(node_a.uniforms().get("count"), Some(&UniformValue::UInt(3)));

        let rotated = scene.get_node(b).unwrap().get_transform() * vec4(1.0, 0.0, 0.0, 1.0);
        assert!(
            (rotated - vec4(0.0, 1.0, 0.0, 1.0)).magnitude() < 1e-6,
            "{rotated:?}"
        );
    }

    #[test]
    fn texture_rect_uniform_names_are_read() {
        let mut assets = assets();
        assets.assets.insert(
            "types.vert.spv".to_string(),
            include_bytes!(concat!(env!("OUT_DIR"), "/shaders/types.vert.spv")).to_vec(),
        );
        assets.assets.insert(
            "types.frag.spv".to_string(),
            include_bytes!(concat!(env!("OUT_DIR"), "/shaders/types.frag.spv")).to_vec(),
        );
        // `time` is the only float uniform of the types shaders.
        let json = r#"{
            "shaders": [{ "name": "S", "vertex": "types.vert.spv", "fragment": "types.frag.spv",
                          "umin_uniform": "time", "umax_uniform": "time",
                          "vmin_uniform": "time", "vmax_uniform": "time" }],
            "root": {}
        }"#;
        let scene = Scene::from_json(&assets, json).unwrap();
        let names = scene.shaders.get_by_name("S").unwrap().get_uniform_names();
        assert_eq!(
            names,
            &ShaderUniformNames::default().with_texture_rect("time", "time", "time", "time")
        );
    }

    #[test]
    fn minimal_scene_has_only_a_root() {
        let scene = Scene::from_json(&assets(), r#"{ "root": {} }"#).unwrap();
        assert_eq!(scene.nodes.iter().count(), 1);
        let root = scene.nodes.iter().next().unwrap();
        assert!(root.is_root());
        assert_eq!(root.get_transform(), Mat4::identity());
    }

    #[test]
    fn every_uniform_type_is_read() {
        let json = r#"{ "root": { "uniforms": {
            "f": { "float": 1.5 }, "i": { "int": -2 }, "u": { "uint": 3 }, "b": { "bool": true },
            "v2": { "vec2": [1, 2] }, "v3": { "vec3": [1, 2, 3] }, "v4": { "vec4": [1, 2, 3, 4] },
            "i2": { "ivec2": [1, -2] }, "i3": { "ivec3": [1, 2, 3] }, "i4": { "ivec4": [1, 2, 3, 4] },
            "u2": { "uvec2": [1, 2] }, "u3": { "uvec3": [1, 2, 3] }, "u4": { "uvec4": [1, 2, 3, 4] },
            "m2": { "mat2": [1, 2, 3, 4] },
            "m3": { "mat3": [1, 2, 3, 4, 5, 6, 7, 8, 9] },
            "m4": { "mat4": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 5, 6, 7, 1] }
        } } }"#;
        let scene = Scene::from_json(&assets(), json).unwrap();
        let uniforms = scene.nodes.iter().next().unwrap().uniforms();
        assert_eq!(uniforms.len(), 16);
        assert_eq!(uniforms.get("f"), Some(&UniformValue::Float(1.5)));
        assert_eq!(
            uniforms.get("i2"),
            Some(&UniformValue::IVec2(IVec2::new(1, -2)))
        );
        assert_eq!(
            uniforms.get("m2"),
            Some(&UniformValue::Mat2(Matrix2::new(1.0, 2.0, 3.0, 4.0)))
        );
        assert_eq!(
            uniforms.get("m4"),
            Some(&UniformValue::Mat4(Mat4::from_translation(Vec3::new(
                5.0, 6.0, 7.0
            ))))
        );
    }

    fn load_error(json: &str) -> String {
        format!("{:#}", Scene::from_json(&assets(), json).err().unwrap())
    }

    #[test]
    fn mesh_files_by_extension() {
        let json = |file: &str| {
            format!(r#"{{ "meshes": [{{ "name": "M", "file": "{file}" }}], "root": {{}} }}"#)
        };
        let scene = Scene::from_json(&assets(), &json("triangle.fmesh")).unwrap();
        // testdata/triangle.fmesh's second vertex is (1.5, 0, 0), TRIANGLE's (1, 0, 0).
        assert_eq!(
            scene.meshes.get_by_name("M").unwrap().get_vertices()[1]
                .pos
                .x,
            1.5
        );
        let scene = Scene::from_json(&assets(), &json("triangle.obj")).unwrap();
        assert_eq!(
            scene.meshes.get_by_name("M").unwrap().get_vertices()[1]
                .pos
                .x,
            1.0
        );
        assert!(load_error(&json("triangle.ply")).contains("unknown file type"));
        assert!(load_error(&json("triangle")).contains("unknown file type"));
    }

    #[test]
    fn loads_shadows() {
        let json = r#"{
            "meshes": [{ "name": "Triangle", "file": "triangle.obj" }],
            "shaders": [{
                "name": "Shadowed", "vertex": "basic.vert.spv", "fragment": "basic.frag.spv",
                "model_uniform": "model", "shadow_matrix_uniform": "view",
                "shadow_map_sampler": "texSampler"
            }],
            "shadow_light": { "direction": [0, 0, -1], "radius": 4, "map_size": 512 },
            "root": {
                "name": "Root",
                "children": [
                    { "name": "Caster", "mesh": "Triangle", "casts_shadow": true },
                    { "name": "Receiver", "mesh": "Triangle", "culled": false }
                ]
            }
        }"#;
        let scene = Scene::from_json(&assets(), json).unwrap();
        let light = scene.get_shadow_light().unwrap();
        assert_eq!(light.map_size, 512);
        let expected =
            ShadowLight::directional(Vec3::new(0.0, 0.0, -1.0), Point3::new(0.0, 0.0, 0.0), 4.0);
        assert_eq!(light.view_projection, expected.view_projection);

        let node = |name| scene.get_node(scene.find_node(name).unwrap()).unwrap();
        assert!(node("Caster").casts_shadow());
        assert!(!node("Receiver").casts_shadow());
        assert!(node("Caster").is_culled());
        assert!(!node("Receiver").is_culled());

        let shader = scene.shaders.get_by_name("Shadowed").unwrap();
        assert_eq!(shader.get_uniform_shadow_matrix_name(), Some("view"));
        assert_eq!(shader.get_shadow_map_sampler_name(), Some("texSampler"));

        // Without `shadow_light`, no shadows.
        assert!(Scene::from_json(&assets(), SCENE)
            .unwrap()
            .get_shadow_light()
            .is_none());
    }

    #[test]
    fn shadow_errors_name_the_problem() {
        let shader = |names: &str| {
            format!(
                r#"{{ "shaders": [{{ "name": "S", "vertex": "basic.vert.spv",
                    "fragment": "basic.frag.spv", {names} }}], "root": {{ "name": "R" }} }}"#
            )
        };
        for (json, expected) in [
            (shader(r#""shadow_map_sampler": "nope""#), "no sampler `nope`"),
            (shader(r#""shadow_map_sampler": "view""#), "no sampler `view`"),
            (shader(r#""shadow_matrix_uniform": "texSampler""#), "is a sampler"),
            (
                r#"{ "shadow_light": { "direction": [0, 0, -1], "radius": 0 }, "root": { "name": "R" } }"#
                    .to_string(),
                "radius must be above 0",
            ),
        ] {
            let error = format!("{:#}", Scene::from_json(&assets(), &json).err().unwrap());
            assert!(error.contains(expected), "{error}");
        }
    }

    /// A scene with `cameras` (the JSON of the array's items) and other top-level `extra`.
    fn with_cameras(cameras: &str, extra: &str) -> String {
        format!(r#"{{ "cameras": [{cameras}], {extra} "root": {{ "name": "R" }} }}"#)
    }

    #[test]
    fn loads_cameras() {
        use crate::camera::{Projection, ScreenOrigin, ScreenUnits};
        let json = with_cameras(
            r#"{ "name": "Persp", "perspective": { "fov": 60 }, "eye": [1, 2, 3],
                 "target": [0, 0, 1], "up": [0, 0, 1], "near": 0.5, "far": 100 },
               { "name": "Flat", "orthographic": { "origin": "top_left", "height": 12 },
                 "near": -10, "far": 10 },
               { "orthographic": {} },
               { "perspective": {} }"#,
            "",
        );
        let scene = Scene::from_json(&assets(), &json).unwrap();
        assert_eq!(scene.cameras.iter().count(), 4);

        let persp = scene
            .get_camera(scene.find_camera("Persp").unwrap())
            .unwrap();
        assert_eq!(persp.get_projection_kind(), Projection::Perspective);
        assert_eq!(persp.get_fov(), 60.0);
        assert_eq!((persp.get_near(), persp.get_far()), (0.5, 100.0));
        assert_eq!(persp.get_eye_position(), Point3::new(1.0, 2.0, 3.0));
        assert_eq!(persp.get_target_position(), Point3::new(0.0, 0.0, 1.0));
        assert_eq!(persp.get_up_vector(), Vec3::new(0.0, 0.0, 1.0));

        let flat = scene
            .get_camera(scene.find_camera("Flat").unwrap())
            .unwrap();
        assert_eq!(flat.get_projection_kind(), Projection::Orthographic);
        assert_eq!(flat.get_origin(), ScreenOrigin::TopLeft);
        assert_eq!(flat.get_units(), ScreenUnits::Height(12.0));
        assert_eq!((flat.get_near(), flat.get_far()), (-10.0, 10.0));

        // Defaults: those of `Camera::orthographic` and `Camera::perspective`.
        let mut unnamed = scene.cameras.iter().filter(|c| c.get_name().is_none());
        let flat = unnamed.next().unwrap();
        let default_flat = Camera::orthographic(ScreenOrigin::Center, ScreenUnits::Pixels);
        assert_eq!(flat.get_origin(), ScreenOrigin::Center);
        assert_eq!(flat.get_units(), ScreenUnits::Pixels);
        assert_eq!(flat.get_view(), default_flat.get_view());
        assert_eq!(flat.get_projection(), default_flat.get_projection());
        let persp = unnamed.next().unwrap();
        let default_persp = Camera::perspective();
        assert_eq!(persp.get_view(), default_persp.get_view());
        assert_eq!(persp.get_projection(), default_persp.get_projection());
    }

    #[test]
    fn first_camera_is_active_unless_active_camera_says_otherwise() {
        let cameras = r#"{ "name": "A", "perspective": {} }, { "name": "B", "perspective": {} }"#;
        let scene = Scene::from_json(&assets(), &with_cameras(cameras, "")).unwrap();
        assert_eq!(scene.active_camera_handle(), scene.find_camera("A"));

        let json = with_cameras(cameras, r#""active_camera": "B","#);
        let scene = Scene::from_json(&assets(), &json).unwrap();
        assert_eq!(scene.active_camera_handle(), scene.find_camera("B"));

        // No cameras: no active camera.
        let scene = Scene::from_json(&assets(), SCENE).unwrap();
        assert!(scene.active_camera().is_none());
    }

    #[test]
    fn camera_errors_name_the_problem() {
        let p = r#""perspective": {}"#;
        let o = r#""orthographic": {}"#;
        for (camera, extra, expected) in [
            (
                format!("{{ {p}, {o} }}"),
                "",
                "exactly one of `perspective` and `orthographic`",
            ),
            (
                "{}".to_string(),
                "",
                "exactly one of `perspective` and `orthographic`",
            ),
            (
                r#"{ "perspective": { "fov": 180 } }"#.into(),
                "",
                "fov must be between 0 and 180",
            ),
            (
                r#"{ "perspective": { "fov": 0 } }"#.into(),
                "",
                "fov must be between 0 and 180",
            ),
            (
                r#"{ "orthographic": { "height": 0 } }"#.into(),
                "",
                "height must be above 0",
            ),
            (
                r#"{ "orthographic": { "origin": "top" } }"#.into(),
                "",
                "unknown variant `top`",
            ),
            (
                format!(r#"{{ {p}, "near": 5, "far": 5 }}"#),
                "",
                "must be below far",
            ),
            (
                format!(r#"{{ {p}, "near": -1 }}"#),
                "",
                "near must be above 0",
            ),
            (
                format!(r#"{{ {p}, "eye": [1, 1, 1], "target": [1, 1, 1] }}"#),
                "",
                "must differ",
            ),
            (
                format!(r#"{{ {p}, "up": [0, 0, 0] }}"#),
                "",
                "up must not be zero",
            ),
            (
                format!(r#"{{ {p}, "zoom": 2 }}"#),
                "",
                "unknown field `zoom`",
            ),
            (
                format!(r#"{{ "name": "C", {p} }}, {{ "name": "C", {o} }}"#),
                "",
                "more than one camera named `C`",
            ),
            (
                format!("{{ {p} }}"),
                r#""active_camera": "Nope","#,
                "unknown camera `Nope`",
            ),
        ] {
            let error = load_error(&with_cameras(&camera, extra));
            assert!(error.contains(expected), "{camera}: {error}");
        }
        // Errors name the camera, or its index.
        let error = load_error(&with_cameras(
            r#"{ "name": "Cam", "orthographic": { "height": -1 } }"#,
            "",
        ));
        assert!(error.contains("camera `Cam`"), "{error}");
        let error = load_error(&with_cameras(
            &format!(r#"{{ {p} }}, {{ {o}, "far": -2000 }}"#),
            "",
        ));
        assert!(error.contains("camera 1"), "{error}");
    }

    #[test]
    fn errors_name_the_problem() {
        let cases = [
            (r#"{ "root": {}, "extra": 1 }"#, "unknown field `extra`"),
            (
                r#"{ "root": {}, "textures": [] }"#,
                "unknown field `textures`",
            ),
            (r#"{ "root": { "mesh": "Nope" } }"#, "unknown mesh `Nope`"),
            (
                r#"{ "root": { "material": "Nope" } }"#,
                "unknown material `Nope`",
            ),
            (
                r#"{ "materials": [{ "name": "M", "shader": "Nope" }], "root": {} }"#,
                "material `M`: unknown shader `Nope`",
            ),
            (
                r#"{ "shaders": [{ "name": "S", "vertex": "basic.vert.spv",
                                   "fragment": "basic.frag.spv" }],
                     "materials": [{ "name": "M", "shader": "S", "texture": { "atlas": "Nope" } }],
                     "root": {} }"#,
                "material `M`: unknown atlas `Nope`",
            ),
            (
                r#"{ "atlases": [{ "name": "A", "bitmap": "atlas.png", "metadata": "atlas.atlas" }],
                     "shaders": [{ "name": "S", "vertex": "basic.vert.spv",
                                   "fragment": "basic.frag.spv" }],
                     "materials": [{ "name": "M", "shader": "S",
                                     "texture": { "atlas": "A", "name": "middle" } }],
                     "root": {} }"#,
                "material `M`: atlas `A` has no texture `middle`",
            ),
            (
                r#"{ "root": { "name": "R", "uniforms": { "t": { "texture": "T" } } } }"#,
                "unknown variant `texture`",
            ),
            (
                r#"{ "meshes": [{ "name": "M", "file": "a.obj" }, { "name": "M", "file": "b.obj" }],
                     "root": {} }"#,
                "more than one mesh named `M`",
            ),
            (
                r#"{ "atlases": [{ "name": "A", "bitmap": "missing.png" }], "root": {} }"#,
                "atlas `A`: asset missing.png not found",
            ),
            (
                r#"{ "atlases": [{ "name": "A", "bitmap": "atlas.png", "metadata": "missing.atlas" }],
                     "root": {} }"#,
                "atlas `A`: Cannot read atlas metadata missing.atlas",
            ),
            (
                r#"{ "atlases": [{ "name": "A", "file": "atlas.png" }], "root": {} }"#,
                "unknown field `file`",
            ),
            (
                r#"{ "root": { "transform": { "matrix": [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1],
                                              "scale": 2 } } }"#,
                "can't be combined",
            ),
            (
                r#"{ "root": { "name": "R", "children": [{ "name": "C", "mesh": "X" }] } }"#,
                "node `R`: node `C`: unknown mesh `X`",
            ),
        ];
        for (json, expected) in cases {
            let error = load_error(json);
            assert!(
                error.contains(expected),
                "expected `{expected}` in: {error}"
            );
        }
    }

    #[test]
    fn cuboid_mesh() {
        let json = r#"{
            "meshes": [
                { "name": "Box", "cuboid": { "x": [-1, 1], "y": [-2, 2], "z": [0, 3] } },
                { "name": "Tiled", "cuboid": { "x": [0, 1], "y": [0, 1], "z": [0, 1],
                                              "u": [[0, 0.5], [0.5, 1]], "v": [[0, 1]] } }
            ],
            "root": { "mesh": "Box" }
        }"#;
        let scene = Scene::from_json(&assets(), json).unwrap();

        let mesh = scene.meshes.get_by_name("Box").unwrap();
        // 6 walls, 4 vertices and 2 triangles each.
        assert_eq!(mesh.gen_num_vertexes(), 24);
        assert_eq!(mesh.gen_num_indexes(), 36);
        let xs = mesh.vertices.iter().map(|v| v.pos.x);
        let zs = mesh.vertices.iter().map(|v| v.pos.z);
        assert_eq!(xs.clone().fold(f32::MAX, f32::min), -1.0);
        assert_eq!(xs.fold(f32::MIN, f32::max), 1.0);
        assert_eq!(zs.fold(f32::MIN, f32::max), 3.0);
        assert!(scene.meshes.get_by_name("Tiled").is_some());
    }

    #[test]
    fn empty_cuboid_is_the_default_unit_cube() {
        let json = r#"{
            "meshes": [
                { "name": "Unit", "cuboid": {} },
                { "name": "Tall", "cuboid": { "z": [0, 2] } }
            ],
            "root": {}
        }"#;
        let scene = Scene::from_json(&assets(), json).unwrap();

        let positions = |name| -> Vec<[f32; 3]> {
            let mesh = scene.meshes.get_by_name(name).unwrap();
            mesh.vertices
                .iter()
                .map(|v| [v.pos.x, v.pos.y, v.pos.z])
                .collect()
        };
        let mut default_scene = Scene::new();
        let default_mesh = MeshBuilderCuboid::default()
            .build(&mut default_scene, None)
            .unwrap();
        let expected: Vec<[f32; 3]> = default_scene
            .get_mesh(default_mesh)
            .unwrap()
            .vertices
            .iter()
            .map(|v| [v.pos.x, v.pos.y, v.pos.z])
            .collect();
        assert_eq!(positions("Unit"), expected);

        // Missing axes default to [-0.5, 0.5].
        for [x, y, z] in positions("Tall") {
            assert!(x == -0.5 || x == 0.5);
            assert!(y == -0.5 || y == 0.5);
            assert!(z == 0.0 || z == 2.0);
        }
    }

    #[test]
    fn plane_mesh() {
        let json = r#"{
            "meshes": [
                { "name": "Unit", "plane": {} },
                { "name": "Grid", "plane": { "x": [0, 2], "y": [-1, 0], "segments": [4, 2] } }
            ],
            "root": {}
        }"#;
        let scene = Scene::from_json(&assets(), json).unwrap();

        let unit = scene.meshes.get_by_name("Unit").unwrap();
        assert_eq!((unit.gen_num_vertexes(), unit.gen_num_indexes()), (4, 6));
        for v in &unit.vertices {
            assert!(v.pos.x.abs() == 0.5 && v.pos.y.abs() == 0.5, "{:?}", v.pos);
        }

        let grid = scene.meshes.get_by_name("Grid").unwrap();
        assert_eq!(grid.gen_num_vertexes(), 5 * 3);
        assert_eq!(grid.gen_num_indexes(), 4 * 2 * 6);
        assert_eq!(grid.vertices[0].pos, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(grid.vertices[14].pos, Vec3::new(2.0, -1.0, 0.0));

        let error = load_error(
            r#"{ "meshes": [{ "name": "P", "plane": { "segments": [0, 1] } }],
                                    "root": {} }"#,
        );
        assert!(
            error.contains("mesh `P`: a plane needs at least 1 segment"),
            "{error}"
        );
    }

    #[test]
    fn sphere_mesh() {
        let json = r#"{
            "meshes": [
                { "name": "Default", "sphere": {} },
                { "name": "Globe", "sphere": { "radius": 2, "segments": 8, "rings": 4 } }
            ],
            "root": {}
        }"#;
        let scene = Scene::from_json(&assets(), json).unwrap();

        let mut default_scene = Scene::new();
        let default = MeshBuilderSphere::default()
            .build(&mut default_scene, None)
            .unwrap();
        let expected = default_scene.get_mesh(default).unwrap();
        let mesh = scene.meshes.get_by_name("Default").unwrap();
        assert_eq!(mesh.vertices, expected.vertices);
        assert_eq!(mesh.indices, expected.indices);

        let globe = scene.meshes.get_by_name("Globe").unwrap();
        assert_eq!(globe.gen_num_vertexes(), 9 * 5);
        use cgmath::InnerSpace;
        assert!((globe.vertices[10].pos.magnitude() - 2.0).abs() < 1e-5);

        let error = load_error(
            r#"{ "meshes": [{ "name": "S", "sphere": { "rings": 1 } }],
                                    "root": {} }"#,
        );
        assert!(
            error.contains("mesh `S`: a sphere needs at least"),
            "{error}"
        );
    }

    #[test]
    fn mesh_needs_exactly_one_source() {
        let both = r#"{ "meshes": [{ "name": "M", "file": "triangle.obj",
                          "cuboid": { "x": [0, 1], "y": [0, 1], "z": [0, 1] } }], "root": {} }"#;
        let cuboid_and_plane =
            r#"{ "meshes": [{ "name": "M", "cuboid": {}, "plane": {} }], "root": {} }"#;
        let plane_and_sphere =
            r#"{ "meshes": [{ "name": "M", "plane": {}, "sphere": {} }], "root": {} }"#;
        let neither = r#"{ "meshes": [{ "name": "M" }], "root": {} }"#;
        for json in [both, cuboid_and_plane, plane_and_sphere, neither] {
            let error = load_error(json);
            assert!(
                error.contains("mesh `M`: a mesh needs exactly one of"),
                "{error}"
            );
        }
    }

    #[test]
    fn load_skips_utf8_byte_order_mark() {
        let mut assets = assets();
        let mut file = "\u{feff}".as_bytes().to_vec();
        file.extend_from_slice(br#"{ "root": { "name": "R" } }"#);
        assets.assets.insert("bom.scene.json".to_string(), file);

        let scene = Scene::load(&assets, "bom.scene.json").unwrap();
        assert!(scene.find_node("R").is_some());
    }

    #[test]
    fn find_node_by_name() {
        let scene = Scene::from_json(&assets(), SCENE).unwrap();
        assert!(scene.find_node("A").is_some());
        assert_eq!(scene.find_node("Missing"), None);
    }
}
