//! Loading a scene from a JSON file.
//!
//! The file lists the scene's atlases, textures, meshes, shaders and materials, and describes the
//! node tree starting from the root. Objects refer to each other by name; files (PNG, OBJ,
//! SPIR-V) are asset names read through an `AssetsProvider`. Example:
//!
//! ```json
//! {
//!   "atlases":  [{ "name": "Room", "file": "viking_room.png" }],
//!   "textures": [{ "name": "Room", "atlas": "Room" },
//!                { "name": "Corner", "atlas": "Room", "rect": [0.0, 0.0, 0.5, 0.5] }],
//!   "meshes":   [{ "name": "Room", "file": "viking_room.obj" }],
//!   "shaders":  [{ "name": "Basic", "vertex": "shader.vert.spv", "fragment": "shader.frag.spv",
//!                  "model_uniform": "model", "view_uniform": "view",
//!                  "projection_uniform": "proj" }],
//!   "materials": [{ "name": "Room", "shader": "Basic",
//!                   "uniforms": { "texSampler": { "texture": "Room" } } }],
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
//! - Every list item needs a name, unique within its list. All lists are optional.
//! - A texture without `rect` covers its whole atlas and becomes the atlas's full texture;
//!   `rect` is `[u_min, v_min, u_range, v_range]` in UV space.
//! - Uniform values are written as `{ "<glsl type>": value }`: `float`, `int`, `uint`, `bool`,
//!   `vec2`-`vec4`, `ivec2`-`ivec4`, `uvec2`-`uvec4` (arrays), `mat2`-`mat4` (column-major
//!   arrays of 4, 9 or 16 numbers), or `texture` (a texture name).
//! - A node transform is either `matrix` (16 numbers, column-major) or any of `translation`
//!   `[x, y, z]`, `rotation` `[x, y, z]` in degrees (applied around X, then Y, then Z) and
//!   `scale` (a number or `[x, y, z]`). Nodes may also set `"visible": false`.
//! - Unknown fields are errors, so typos don't go unnoticed.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{anyhow, bail, Context, Result};
use cgmath::{Deg, Matrix2, Matrix3, Matrix4};
use serde::Deserialize;

use super::assetsprovider::AssetsProvider;
use super::math::{IVec2, IVec3, IVec4, Mat4, UVec2, UVec3, UVec4, Vec2, Vec3, Vec4};
use super::meshbuilderobjfile::MeshBuilderObjFile;
use super::scene::Scene;
use super::texture::Texture;
use super::uniform::UniformValue;
use super::uniforms::Uniforms;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneFile {
    #[serde(default)]
    atlases: Vec<AtlasEntry>,
    #[serde(default)]
    textures: Vec<TextureEntry>,
    #[serde(default)]
    meshes: Vec<MeshEntry>,
    #[serde(default)]
    shaders: Vec<ShaderEntry>,
    #[serde(default)]
    materials: Vec<MaterialEntry>,
    root: NodeEntry,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AtlasEntry {
    name: String,
    /// PNG asset.
    file: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextureEntry {
    name: String,
    atlas: String,
    /// `[u_min, v_min, u_range, v_range]`; the whole atlas when missing.
    rect: Option<[f32; 4]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeshEntry {
    name: String,
    /// OBJ asset.
    file: String,
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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialEntry {
    name: String,
    shader: String,
    #[serde(default)]
    uniforms: BTreeMap<String, UniformEntry>,
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
    uniforms: BTreeMap<String, UniformEntry>,
    #[serde(default)]
    children: Vec<NodeEntry>,
}

fn default_visible() -> bool {
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
    /// Name of a texture.
    Texture(String),
}

impl Scene {
    /// Loads a scene from the JSON asset `asset`; see the `scenefile` module for the format.
    /// Files named in the scene are read through `assets` too.
    pub fn load(assets: &dyn AssetsProvider, asset: &str) -> Result<Scene> {
        let json = assets.get_asset(asset)?;
        let json = std::str::from_utf8(&json).with_context(|| format!("{asset} is not UTF-8"))?;
        Self::from_json(assets, json).with_context(|| format!("Cannot load scene {asset}"))
    }

    /// Builds a scene from JSON text; see the `scenefile` module for the format.
    pub fn from_json(assets: &dyn AssetsProvider, json: &str) -> Result<Scene> {
        let file: SceneFile = serde_json::from_str(json)?;
        let mut scene = Scene::new();

        check_unique("atlas", file.atlases.iter().map(|a| &a.name))?;
        check_unique("texture", file.textures.iter().map(|t| &t.name))?;
        check_unique("mesh", file.meshes.iter().map(|m| &m.name))?;
        check_unique("shader", file.shaders.iter().map(|s| &s.name))?;
        check_unique("material", file.materials.iter().map(|m| &m.name))?;

        let mut atlases = HashMap::new();
        for entry in &file.atlases {
            let handle = scene
                .create_atlas_from_png(assets, &entry.file, Some(entry.name.clone()))
                .with_context(|| format!("atlas `{}`", entry.name))?;
            atlases.insert(entry.name.as_str(), handle);
        }

        let mut textures = HashMap::new();
        for entry in &file.textures {
            let what = || format!("texture `{}`", entry.name);
            let atlas = *atlases
                .get(entry.atlas.as_str())
                .ok_or_else(|| anyhow!("unknown atlas `{}`", entry.atlas))
                .with_context(what)?;
            let name = Some(entry.name.clone());
            let handle = match entry.rect {
                None => scene.create_full_atlas_texture(name, atlas),
                Some([u, v, w, h]) => scene.add_texture(Texture::new(name, atlas, u, v, w, h)),
            }
            .with_context(what)?;
            textures.insert(entry.name.as_str(), handle);
        }

        let mut meshes = HashMap::new();
        for entry in &file.meshes {
            let handle = MeshBuilderObjFile::new(assets, &entry.file)
                .and_then(|b| b.with_name(Some(entry.name.clone())).build(&mut scene))
                .with_context(|| format!("mesh `{}`", entry.name))?;
            meshes.insert(entry.name.as_str(), handle);
        }

        let mut shaders = HashMap::new();
        for entry in &file.shaders {
            let handle = (|| {
                scene.create_shader(
                    Some(entry.name.clone()),
                    &assets.get_asset(&entry.vertex)?,
                    &assets.get_asset(&entry.fragment)?,
                    entry.model_uniform.clone(),
                    entry.view_uniform.clone(),
                    entry.projection_uniform.clone(),
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
                let uniforms = scene.get_material_mut(material).unwrap().uniforms_mut();
                set_uniforms(uniforms, &entry.uniforms, &textures)?;
                Ok::<_, anyhow::Error>(material)
            })()
            .with_context(|| format!("material `{}`", entry.name))?;
            materials.insert(entry.name.as_str(), handle);
        }

        let names = Names {
            meshes,
            materials,
            textures,
        };
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

/// Handles of the scene's named objects that nodes refer to.
struct Names<'a> {
    meshes: HashMap<&'a str, usize>,
    materials: HashMap<&'a str, usize>,
    textures: HashMap<&'a str, usize>,
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
        set_uniforms(node.uniforms_mut(), &entry.uniforms, &names.textures)
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

fn set_uniforms(
    uniforms: &mut Uniforms,
    entries: &BTreeMap<String, UniformEntry>,
    textures: &HashMap<&str, usize>,
) -> Result<()> {
    for (name, entry) in entries {
        let value = entry
            .to_value(textures)
            .with_context(|| format!("uniform `{name}`"))?;
        uniforms.set(name.clone(), value);
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
    fn to_value(&self, textures: &HashMap<&str, usize>) -> Result<UniformValue> {
        use UniformEntry as E;
        Ok(match *self {
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
            E::Texture(ref name) => UniformValue::Texture(
                *textures
                    .get(name.as_str())
                    .ok_or_else(|| anyhow!("unknown texture `{name}`"))?,
            ),
        })
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
        let rotation = Mat4::from_angle_z(Deg(z)) * Mat4::from_angle_y(Deg(y)) * Mat4::from_angle_x(Deg(x));
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
    use crate::feather::assetsprovider::tests::MemoryAssetsProvider;
    use crate::feather::object::Object;
    use crate::feather::shader::tests::{FRAG, VERT};
    use cgmath::{vec4, InnerSpace, SquareMatrix};

    const TRIANGLE: &str = "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nvn 0 0 1\nf 1/1/1 2/2/1 3/3/1\n";

    fn png_2x2() -> Vec<u8> {
        let mut png = Vec::new();
        let mut encoder = png::Encoder::new(&mut png, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[255; 16]).unwrap();
        writer.finish().unwrap();
        png
    }

    fn assets() -> MemoryAssetsProvider {
        let mut assets = MemoryAssetsProvider::default();
        for (name, bytes) in [
            ("atlas.png", png_2x2()),
            ("triangle.obj", TRIANGLE.as_bytes().to_vec()),
            ("basic.vert.spv", VERT.to_vec()),
            ("basic.frag.spv", FRAG.to_vec()),
        ] {
            assets.assets.insert(name.to_string(), bytes);
        }
        assets
    }

    const SCENE: &str = r#"{
        "atlases": [{ "name": "Atlas", "file": "atlas.png" }],
        "textures": [
            { "name": "Full", "atlas": "Atlas" },
            { "name": "Corner", "atlas": "Atlas", "rect": [0.5, 0.0, 0.5, 0.25] }
        ],
        "meshes": [{ "name": "Triangle", "file": "triangle.obj" }],
        "shaders": [{
            "name": "Basic", "vertex": "basic.vert.spv", "fragment": "basic.frag.spv",
            "model_uniform": "model", "view_uniform": "view", "projection_uniform": "proj"
        }],
        "materials": [{
            "name": "Mat", "shader": "Basic",
            "uniforms": { "texSampler": { "texture": "Full" }, "tint": { "vec4": [1, 0.5, 0.25, 1] } }
        }],
        "root": {
            "name": "Root",
            "children": [
                {
                    "name": "A", "mesh": "Triangle", "material": "Mat",
                    "transform": { "translation": [1, 2, 3], "scale": 2 },
                    "uniforms": { "texSampler": { "texture": "Corner" }, "count": { "uint": 3 } },
                    "children": [{ "name": "A1", "visible": false }]
                },
                { "name": "B", "transform": { "rotation": [0, 0, 90] } }
            ]
        }
    }"#;

    #[test]
    fn loads_every_kind_of_object() {
        let scene = Scene::from_json(&assets(), SCENE).unwrap();

        let atlas = scene.atlases.get_by_name("Atlas").unwrap();
        assert_eq!((atlas.get_width(), atlas.get_height()), (2, 2));

        let full = scene.textures.get_by_name("Full").unwrap();
        assert_eq!(full.get_atlas(), atlas.get_handle());
        assert_eq!(atlas.get_full_texture(), Some(full.get_handle()));
        let corner = scene.textures.get_by_name("Corner").unwrap();
        assert_eq!(corner.get_uv_rect(), vec4(0.5, 0.0, 0.5, 0.25));

        let mesh = scene.meshes.get_by_name("Triangle").unwrap();
        assert_eq!(mesh.gen_num_indexes(), 3);

        let shader = scene.shaders.get_by_name("Basic").unwrap();
        assert_eq!(shader.get_uniform_proj_name(), Some("proj"));

        let material = scene.materials.get_by_name("Mat").unwrap();
        assert_eq!(material.get_shader(), shader.get_handle());
        assert_eq!(
            material.uniforms().get("texSampler"),
            Some(&UniformValue::Texture(full.get_handle()))
        );
        assert_eq!(
            material.uniforms().get("tint"),
            Some(&UniformValue::Vec4(vec4(1.0, 0.5, 0.25, 1.0)))
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

        let node_a = scene.get_node(a).unwrap();
        assert_eq!(node_a.get_mesh(), Some(scene.meshes.get_by_name("Triangle").unwrap().get_handle()));
        assert_eq!(node_a.get_material(), Some(scene.materials.get_by_name("Mat").unwrap().get_handle()));
        assert_eq!(
            node_a.get_transform(),
            Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)) * Mat4::from_scale(2.0)
        );
        let corner = scene.textures.get_by_name("Corner").unwrap().get_handle();
        assert_eq!(node_a.uniforms().get("texSampler"), Some(&UniformValue::Texture(corner)));
        assert_eq!(node_a.uniforms().get("count"), Some(&UniformValue::UInt(3)));

        let rotated = scene.get_node(b).unwrap().get_transform() * vec4(1.0, 0.0, 0.0, 1.0);
        assert!((rotated - vec4(0.0, 1.0, 0.0, 1.0)).magnitude() < 1e-6, "{rotated:?}");
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
        assert_eq!(uniforms.get("i2"), Some(&UniformValue::IVec2(IVec2::new(1, -2))));
        assert_eq!(
            uniforms.get("m2"),
            Some(&UniformValue::Mat2(Matrix2::new(1.0, 2.0, 3.0, 4.0)))
        );
        assert_eq!(
            uniforms.get("m4"),
            Some(&UniformValue::Mat4(Mat4::from_translation(Vec3::new(5.0, 6.0, 7.0))))
        );
    }

    fn load_error(json: &str) -> String {
        format!("{:#}", Scene::from_json(&assets(), json).err().unwrap())
    }

    #[test]
    fn errors_name_the_problem() {
        let cases = [
            (r#"{ "root": {}, "extra": 1 }"#, "unknown field `extra`"),
            (r#"{ "root": { "mesh": "Nope" } }"#, "unknown mesh `Nope`"),
            (r#"{ "root": { "material": "Nope" } }"#, "unknown material `Nope`"),
            (
                r#"{ "textures": [{ "name": "T", "atlas": "Nope" }], "root": {} }"#,
                "texture `T`: unknown atlas `Nope`",
            ),
            (
                r#"{ "materials": [{ "name": "M", "shader": "Nope" }], "root": {} }"#,
                "material `M`: unknown shader `Nope`",
            ),
            (
                r#"{ "root": { "name": "R", "uniforms": { "t": { "texture": "Nope" } } } }"#,
                "node `R`: uniform `t`: unknown texture `Nope`",
            ),
            (
                r#"{ "meshes": [{ "name": "M", "file": "a.obj" }, { "name": "M", "file": "b.obj" }],
                     "root": {} }"#,
                "more than one mesh named `M`",
            ),
            (
                r#"{ "atlases": [{ "name": "A", "file": "missing.png" }], "root": {} }"#,
                "atlas `A`: asset missing.png not found",
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
            assert!(error.contains(expected), "expected `{expected}` in: {error}");
        }
    }

    #[test]
    fn viking_room_scene_loads() {
        // The assets the app gets: resources/ plus the compiled shaders.
        let mut assets = MemoryAssetsProvider::default();
        let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        for entry in std::fs::read_dir(resources).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            assets.assets.insert(name, std::fs::read(&path).unwrap());
        }
        assets.assets.insert("shader.vert.spv".to_string(), VERT.to_vec());
        assets.assets.insert("shader.frag.spv".to_string(), FRAG.to_vec());

        let scene = Scene::load(&assets, "viking_room.scene.json").unwrap();

        let room = scene.get_node(scene.find_node("Room").unwrap()).unwrap();
        assert!(room.get_mesh().is_some());
        assert!(room.get_material().is_some());
        assert_eq!(scene.get_node(room.get_parent().unwrap()).unwrap().get_name(), Some("Scene root".to_string()));
    }

    #[test]
    fn find_node_by_name() {
        let scene = Scene::from_json(&assets(), SCENE).unwrap();
        assert!(scene.find_node("A").is_some());
        assert_eq!(scene.find_node("Missing"), None);
    }
}
