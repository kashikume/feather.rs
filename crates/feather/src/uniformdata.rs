//! Turning uniform values into what a shader reads: bytes for uniform blocks (one aligned
//! slice per block in a per-frame buffer) and push constants, and an atlas for each sampler.

use super::math::Mat4;
use super::shader::ShaderUniformIds;
use super::shaderlayout::{ShaderLayout, UniformMember, UniformType};
use super::texture::Texture;
use super::uniform::UniformValue;
use super::uniformname::{UniformKey, UniformName};
use super::uniforms::Uniforms;

/// The values available to one draw. `engine` values (the matrices and the texture rectangle
/// under the shader's names, see `engine_uniforms`) take precedence over the node's dynamic
/// uniforms.
pub struct UniformValues<'a> {
    pub engine: &'a [(UniformName, UniformValue)],
    pub node: &'a Uniforms,
}

impl UniformValues<'_> {
    pub fn get(&self, name: impl UniformKey) -> Option<&UniformValue> {
        let name = name.uniform_name()?;
        self.engine
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v)
            .or_else(|| self.node.get(name))
    }
}

/// Something wrong with the uniforms of a draw. The draw still happens; the affected uniform
/// keeps zero bytes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum UniformIssue {
    /// The shader uses a uniform that has no value.
    Missing(String),
    /// The value doesn't match the uniform's type in the shader.
    WrongType {
        name: String,
        expected: UniformType,
        found: &'static str,
    },
    /// A value is set for a uniform the shader doesn't have.
    Unused(String),
    /// The shader has samplers, but the material has no texture.
    NoTexture,
    /// The material's texture belongs to an atlas that isn't in the scene.
    UnknownAtlas(usize),
}

impl std::fmt::Display for UniformIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UniformIssue::Missing(name) => write!(f, "uniform `{name}` has no value"),
            UniformIssue::WrongType {
                name,
                expected,
                found,
            } => write!(f, "uniform `{name}` is {expected:?} in the shader, but the value is {found}"),
            UniformIssue::Unused(name) => write!(f, "uniform `{name}` is not used by the shader"),
            UniformIssue::NoTexture => {
                write!(f, "the shader has samplers, but the material has no texture")
            }
            UniformIssue::UnknownAtlas(atlas) => {
                write!(f, "the material's texture uses atlas {atlas}, which doesn't exist")
            }
        }
    }
}

/// Writes the value of each member into `block` (the bytes of one block, zeroed by the caller).
pub fn fill_members(
    block: &mut [u8],
    members: &[UniformMember],
    values: &UniformValues,
    issues: &mut Vec<UniformIssue>,
) {
    for member in members {
        match values.get(member.name) {
            None => issues.push(UniformIssue::Missing(member.name.to_string())),
            Some(value) if !member.ty.accepts(value) => issues.push(UniformIssue::WrongType {
                name: member.name.to_string(),
                expected: member.ty,
                found: value.glsl_type(),
            }),
            Some(value) => {
                let written =
                    value.write_bytes(block, member.offset as usize, stride(member));
                debug_assert!(written, "uniform `{}` doesn't fit its block", member.name);
            }
        }
    }
}

fn stride(member: &UniformMember) -> usize {
    member.matrix_stride.unwrap_or(0) as usize
}

/// Bytes of all uniform blocks drawn in one frame. Each block starts at a multiple of
/// `alignment` (the device's `minUniformBufferOffsetAlignment`), so it can be bound with a
/// dynamic offset.
pub struct UniformArena {
    data: Vec<u8>,
    alignment: usize,
}

impl UniformArena {
    pub fn new(alignment: usize) -> Self {
        Self {
            data: Vec::new(),
            alignment: alignment.max(1),
        }
    }

    /// Reserves `size` zeroed bytes and returns their offset.
    pub fn alloc(&mut self, size: usize) -> usize {
        let offset = self.data.len().next_multiple_of(self.alignment);
        self.data.resize(offset + size, 0);
        offset
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn bytes_mut(&mut self, offset: usize, size: usize) -> &mut [u8] {
        &mut self.data[offset..offset + size]
    }
}

/// Uniform data of one draw.
#[derive(Debug, Default, PartialEq)]
pub struct PackedDraw {
    /// Offset of each uniform block in the arena, in `ShaderLayout::uniform_blocks` order (the
    /// order Vulkan expects dynamic offsets in).
    pub block_offsets: Vec<u32>,
    /// Bytes of the push constant block (empty if the shader has none).
    pub push_constants: Vec<u8>,
}

/// Packs the uniforms of one draw: every uniform block goes into `arena`, push constants into
/// the result. Samplers are not handled here.
pub fn pack_draw(
    layout: &ShaderLayout,
    values: &UniformValues,
    arena: &mut UniformArena,
    issues: &mut Vec<UniformIssue>,
) -> PackedDraw {
    let mut packed = PackedDraw::default();
    for block in &layout.uniform_blocks {
        let size = block.size as usize;
        let offset = arena.alloc(size);
        fill_members(arena.bytes_mut(offset, size), &block.members, values, issues);
        packed.block_offsets.push(offset as u32);
    }
    if let Some(push) = &layout.push_constants {
        packed.push_constants = vec![0; push.size as usize];
        fill_members(&mut packed.push_constants, &push.members, values, issues);
    }
    issues.extend(unused_uniforms(layout, values.node));
    packed
}

/// The values the engine provides for a draw, under the names the shader gave them (see
/// `ShaderUniformNames`, here as `ShaderUniformIds`): the model, view and projection matrices, the shadow light's matrix
/// and, when the material has a texture, the edges of its rectangle. Names the shader doesn't
/// use are left out.
pub fn engine_uniforms(
    names: &ShaderUniformIds,
    model: Mat4,
    view: Mat4,
    projection: Mat4,
    shadow_matrix: Mat4,
    texture: Option<&Texture>,
) -> Vec<(UniformName, UniformValue)> {
    let matrices = [
        (&names.model, model),
        (&names.view, view),
        (&names.projection, projection),
        (&names.shadow_matrix, shadow_matrix),
    ]
    .into_iter()
    .map(|(name, m)| (name, UniformValue::Mat4(m)));
    let rect = texture.into_iter().flat_map(|t| {
        [
            (&names.umin, t.get_u_min()),
            (&names.umax, t.get_u_max()),
            (&names.vmin, t.get_v_min()),
            (&names.vmax, t.get_v_max()),
        ]
        .into_iter()
        .map(|(name, v)| (name, UniformValue::Float(v)))
    });
    matrices
        .chain(rect)
        .filter_map(|(name, value)| Some(((*name)?, value)))
        .collect()
}

/// What a sampler of a shader samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SamplerSource {
    /// An atlas (by handle), or nothing (the fallback texture).
    Atlas(Option<usize>),
    /// The scene's shadow map.
    ShadowMap,
}

/// What each sampler of the shader samples, in `ShaderLayout::samplers` order: the sampler
/// named `shadow_map` (see `ShaderUniformNames`) the shadow map, every other one the atlas of
/// the material's texture. That is `None` (and an issue) when the material has no texture or
/// its atlas doesn't exist (`atlas_exists`).
pub fn sampler_sources(
    layout: &ShaderLayout,
    shadow_map: Option<UniformName>,
    texture: Option<&Texture>,
    atlas_exists: impl Fn(usize) -> bool,
    issues: &mut Vec<UniformIssue>,
) -> Vec<SamplerSource> {
    let is_shadow = |name: UniformName| shadow_map == Some(name);
    if layout.samplers.iter().all(|s| is_shadow(s.name)) {
        return vec![SamplerSource::ShadowMap; layout.samplers.len()];
    }
    let atlas = match texture {
        None => {
            issues.push(UniformIssue::NoTexture);
            None
        }
        Some(texture) if !atlas_exists(texture.get_atlas()) => {
            issues.push(UniformIssue::UnknownAtlas(texture.get_atlas()));
            None
        }
        Some(texture) => Some(texture.get_atlas()),
    };
    layout
        .samplers
        .iter()
        .map(|s| match is_shadow(s.name) {
            true => SamplerSource::ShadowMap,
            false => SamplerSource::Atlas(atlas),
        })
        .collect()
}

/// Node uniforms the shader has no place for.
pub fn unused_uniforms(layout: &ShaderLayout, uniforms: &Uniforms) -> Vec<UniformIssue> {
    uniforms
        .iter()
        .filter(|u| layout.find(u.name).is_none())
        .map(|u| UniformIssue::Unused(u.name.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Mat4, Vec4};
    use crate::shader::tests::{FRAG, VERT};
    use crate::shader::ShaderUniformNames;
    use cgmath::SquareMatrix;

    fn mat_bytes(m: Mat4) -> Vec<u8> {
        let mut buf = vec![0; 64];
        UniformValue::Mat4(m).write_bytes(&mut buf, 0, 16);
        buf
    }

    /// Engine values with their names interned, as `engine_uniforms` returns them.
    fn named<const N: usize>(values: [(&str, UniformValue); N]) -> Vec<(UniformName, UniformValue)> {
        values.into_iter().map(|(n, v)| (UniformName::new(n), v)).collect()
    }

    fn translation(x: f32) -> Mat4 {
        Mat4::from_translation(crate::math::Vec3::new(x, 0.0, 0.0))
    }

    #[test]
    fn engine_values_override_node_values() {
        let mut node = Uniforms::new();
        node.set("view", Mat4::identity()).set("tint", 1.0f32);
        let engine = named([("view", UniformValue::Mat4(translation(1.0)))]);
        let values = UniformValues {
            engine: &engine,
            node: &node,
        };
        assert_eq!(values.get("view"), Some(&UniformValue::Mat4(translation(1.0))));
        assert_eq!(values.get("tint"), Some(&UniformValue::Float(1.0)));
        assert_eq!(values.get("none"), None);
    }

    #[test]
    fn arena_aligns_blocks() {
        let mut arena = UniformArena::new(256);
        assert_eq!(arena.alloc(128), 0);
        assert_eq!(arena.alloc(16), 256);
        assert_eq!(arena.alloc(4), 512);
        assert_eq!(arena.bytes().len(), 516);
        assert!(arena.bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn pack_basic_shader() {
        let layout = ShaderLayout::reflect(VERT, FRAG).unwrap();
        let node = Uniforms::new();
        let engine = named([
            ("model", UniformValue::Mat4(translation(1.0))),
            ("view", UniformValue::Mat4(translation(2.0))),
            ("proj", UniformValue::Mat4(translation(3.0))),
        ]);
        let values = UniformValues {
            engine: &engine,
            node: &node,
        };
        let mut arena = UniformArena::new(256);
        arena.alloc(10); // something drawn before
        let mut issues = Vec::new();

        let packed = pack_draw(&layout, &values, &mut arena, &mut issues);

        assert_eq!(issues, []);
        assert_eq!(packed.block_offsets, [256]);
        let block = &arena.bytes()[256..256 + 128];
        assert_eq!(&block[..64], mat_bytes(translation(2.0)));
        assert_eq!(&block[64..], mat_bytes(translation(3.0)));
        assert_eq!(packed.push_constants, mat_bytes(translation(1.0)));
    }

    #[test]
    fn engine_uniforms_use_the_shader_names() {
        let names = ShaderUniformNames::matrices("model", "view", "proj")
            .with_texture_rect("umin", "umax", "vmin", "vmax")
            .with_shadow("light", "shadows")
            .ids();
        let texture = Texture::new(3, 0.25, 0.5, 0.125, 1.0);
        let values = engine_uniforms(
            &names,
            translation(1.0),
            translation(2.0),
            translation(3.0),
            translation(4.0),
            Some(&texture),
        );
        assert_eq!(
            values,
            named([
                ("model", UniformValue::Mat4(translation(1.0))),
                ("view", UniformValue::Mat4(translation(2.0))),
                ("proj", UniformValue::Mat4(translation(3.0))),
                ("light", UniformValue::Mat4(translation(4.0))),
                ("umin", UniformValue::Float(0.25)),
                ("umax", UniformValue::Float(0.5)),
                ("vmin", UniformValue::Float(0.125)),
                ("vmax", UniformValue::Float(1.0)),
            ])
        );
    }

    #[test]
    fn engine_uniforms_skip_unnamed_values_and_missing_texture() {
        let names = ShaderUniformNames {
            view: Some("v".to_string()),
            umax: Some("right".to_string()),
            ..ShaderUniformNames::default()
        }
        .ids();
        let m = Mat4::identity();
        let texture = Texture::full(0);
        assert_eq!(
            engine_uniforms(&names, m, m, m, m, Some(&texture)),
            named([("v", UniformValue::Mat4(m)), ("right", UniformValue::Float(1.0))])
        );
        // Without a texture there is no rectangle; packing reports the uniform as missing.
        assert_eq!(
            engine_uniforms(&names, m, m, m, m, None),
            named([("v", UniformValue::Mat4(m))])
        );
    }

    #[test]
    fn samplers_get_the_material_texture_atlas() {
        let layout = ShaderLayout::reflect(VERT, FRAG).unwrap();
        let texture = Texture::new(8, 0.5, 1.0, 0.0, 0.25);
        let mut issues = Vec::new();

        let sources = sampler_sources(&layout, None, Some(&texture), |a| a == 8, &mut issues);

        assert_eq!(issues, []);
        assert_eq!(sources, [SamplerSource::Atlas(Some(8))]);
    }

    #[test]
    fn missing_texture_or_atlas_is_reported() {
        let layout = ShaderLayout::reflect(VERT, FRAG).unwrap();
        let mut issues = Vec::new();
        assert_eq!(
            sampler_sources(&layout, None, None, |_| true, &mut issues),
            [SamplerSource::Atlas(None)]
        );
        assert_eq!(issues, [UniformIssue::NoTexture]);

        issues.clear();
        let texture = Texture::full(99);
        assert_eq!(
            sampler_sources(&layout, None, Some(&texture), |_| false, &mut issues),
            [SamplerSource::Atlas(None)]
        );
        assert_eq!(issues, [UniformIssue::UnknownAtlas(99)]);
    }

    #[test]
    fn shaders_without_samplers_need_no_texture() {
        let layout = ShaderLayout::default();
        let mut issues = Vec::new();
        assert!(sampler_sources(&layout, None, None, |_| true, &mut issues).is_empty());
        assert_eq!(issues, []);
    }

    #[test]
    fn the_shadow_map_sampler_samples_the_shadow_map() {
        let layout = ShaderLayout::reflect(VERT, FRAG).unwrap();
        let name = layout.samplers[0].name;
        let mut issues = Vec::new();
        // The shader's only sampler is the shadow map: no texture needed.
        assert_eq!(
            sampler_sources(&layout, Some(name), None, |_| true, &mut issues),
            [SamplerSource::ShadowMap]
        );
        assert_eq!(issues, []);
    }

    #[test]
    fn missing_wrong_and_unused_uniforms_are_reported() {
        let layout = ShaderLayout::reflect(VERT, FRAG).unwrap();
        let mut node = Uniforms::new();
        node.set("view", 1.0f32) // wrong type
            .set("extra", Vec4::new(0.0, 0.0, 0.0, 0.0)); // not in the shader
        let values = UniformValues {
            engine: &[],
            node: &node,
        };
        let mut arena = UniformArena::new(64);
        let mut issues = Vec::new();

        let packed = pack_draw(&layout, &values, &mut arena, &mut issues);

        assert_eq!(
            issues,
            [
                UniformIssue::WrongType {
                    name: "view".to_string(),
                    expected: UniformType::Mat4,
                    found: "float",
                },
                UniformIssue::Missing("proj".to_string()),
                UniformIssue::Missing("model".to_string()),
                UniformIssue::Unused("extra".to_string()),
            ]
        );
        // Uniforms without a valid value stay zero.
        assert!(arena.bytes().iter().all(|&b| b == 0));
        assert!(packed.push_constants.iter().all(|&b| b == 0));
    }
}
