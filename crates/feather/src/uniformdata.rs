//! Turning uniform values into what a shader reads: bytes for uniform blocks (one aligned
//! slice per block in a per-frame buffer) and push constants, and an atlas for each sampler.

use super::material::MAX_MATERIAL_TEXTURES;
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
    /// The shader has a texture sampler for this slot, but the material's slot is empty.
    NoTexture(usize),
    /// The material's texture in this slot belongs to an atlas that isn't in the scene.
    UnknownAtlas { slot: usize, atlas: usize },
}

impl std::fmt::Display for UniformIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UniformIssue::Missing(name) => write!(f, "uniform `{name}` has no value"),
            UniformIssue::WrongType {
                name,
                expected,
                found,
            } => write!(
                f,
                "uniform `{name}` is {expected:?} in the shader, but the value is {found}"
            ),
            UniformIssue::Unused(name) => write!(f, "uniform `{name}` is not used by the shader"),
            UniformIssue::NoTexture(slot) => {
                write!(
                    f,
                    "the shader samples texture slot {slot}, but the material has no texture there"
                )
            }
            UniformIssue::UnknownAtlas { slot, atlas } => {
                write!(
                    f,
                    "the material's texture in slot {slot} uses atlas {atlas}, which doesn't exist"
                )
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
                let written = value.write_bytes(block, member.offset as usize, stride(member));
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
        fill_members(
            arena.bytes_mut(offset, size),
            &block.members,
            values,
            issues,
        );
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
/// `ShaderUniformNames`, here as `ShaderUniformIds`): the model, view and projection matrices,
/// the shadow light's matrix and, for each texture sampler with rectangle uniforms whose slot
/// has a texture, the edges of that texture's rectangle. Names the shader doesn't use are left
/// out.
pub fn engine_uniforms(
    names: &ShaderUniformIds,
    model: Mat4,
    view: Mat4,
    projection: Mat4,
    shadow_matrix: Mat4,
    textures: &[Option<Texture>; MAX_MATERIAL_TEXTURES],
) -> Vec<(UniformName, UniformValue)> {
    let matrices = [
        (names.model, model),
        (names.view, view),
        (names.projection, projection),
        (names.shadow_matrix, shadow_matrix),
    ]
    .into_iter()
    .filter_map(|(name, m)| Some((name?, UniformValue::Mat4(m))));
    let rects = names
        .texture_rects
        .iter()
        .zip(textures)
        .filter_map(|(rect, texture)| Some(((*rect)?, (*texture)?)))
        .flat_map(|(rect, t)| {
            [
                (rect.umin, t.get_u_min()),
                (rect.umax, t.get_u_max()),
                (rect.vmin, t.get_v_min()),
                (rect.vmax, t.get_v_max()),
            ]
            .map(|(name, v)| (name, UniformValue::Float(v)))
        });
    matrices.chain(rects).collect()
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
/// named `shadow_map` the shadow map, and texture sampler `i` (`ShaderUniformIds::
/// texture_samplers`) the atlas of the material's texture in slot `i`. That is `None` (and an
/// issue) when the slot is empty or its atlas doesn't exist (`atlas_exists`). A sampler that is
/// neither (which `Shader::new` doesn't allow) gets `None` too.
pub fn sampler_sources(
    layout: &ShaderLayout,
    names: &ShaderUniformIds,
    textures: &[Option<Texture>; MAX_MATERIAL_TEXTURES],
    atlas_exists: impl Fn(usize) -> bool,
    issues: &mut Vec<UniformIssue>,
) -> Vec<SamplerSource> {
    layout
        .samplers
        .iter()
        .map(|s| {
            if names.shadow_map == Some(s.name) {
                return SamplerSource::ShadowMap;
            }
            let Some(slot) = names
                .texture_samplers
                .iter()
                .position(|n| *n == Some(s.name))
            else {
                return SamplerSource::Atlas(None);
            };
            let atlas = match textures[slot] {
                None => {
                    issues.push(UniformIssue::NoTexture(slot));
                    None
                }
                Some(texture) if !atlas_exists(texture.get_atlas()) => {
                    issues.push(UniformIssue::UnknownAtlas {
                        slot,
                        atlas: texture.get_atlas(),
                    });
                    None
                }
                Some(texture) => Some(texture.get_atlas()),
            };
            SamplerSource::Atlas(atlas)
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
    use crate::shader::tests::{basic_names, FRAG, TWO_TEXTURES_FRAG, VERT};
    use crate::shader::{ShaderUniformIds, ShaderUniformNames, TextureRectNames};
    use cgmath::SquareMatrix;

    fn mat_bytes(m: Mat4) -> Vec<u8> {
        let mut buf = vec![0; 64];
        UniformValue::Mat4(m).write_bytes(&mut buf, 0, 16);
        buf
    }

    /// Engine values with their names interned, as `engine_uniforms` returns them.
    fn named<const N: usize>(
        values: [(&str, UniformValue); N],
    ) -> Vec<(UniformName, UniformValue)> {
        values
            .into_iter()
            .map(|(n, v)| (UniformName::new(n), v))
            .collect()
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
        assert_eq!(
            values.get("view"),
            Some(&UniformValue::Mat4(translation(1.0)))
        );
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

    /// Material texture slots holding `textures` from slot 0 on.
    fn slots<const N: usize>(textures: [Texture; N]) -> [Option<Texture>; MAX_MATERIAL_TEXTURES] {
        let mut slots = [None; MAX_MATERIAL_TEXTURES];
        for (slot, texture) in textures.into_iter().enumerate() {
            slots[slot] = Some(texture);
        }
        slots
    }

    const EMPTY: [Option<Texture>; MAX_MATERIAL_TEXTURES] = [None; MAX_MATERIAL_TEXTURES];

    #[test]
    fn engine_uniforms_use_the_shader_names() {
        let names = ShaderUniformNames::matrices("model", "view", "proj")
            .with_texture_sampler(
                "atlas",
                Some(TextureRectNames::new("umin", "umax", "vmin", "vmax")),
            )
            .with_shadow("light", "shadows")
            .ids();
        let texture = Texture::new(3, 0.25, 0.5, 0.125, 1.0);
        let values = engine_uniforms(
            &names,
            translation(1.0),
            translation(2.0),
            translation(3.0),
            translation(4.0),
            &slots([texture]),
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
    fn engine_uniforms_give_each_slot_its_rectangle() {
        let names = ShaderUniformNames::default()
            .with_texture_sampler("a", Some(TextureRectNames::new("a0", "a1", "a2", "a3")))
            .with_texture_sampler("b", None)
            .with_texture_sampler("c", Some(TextureRectNames::new("c0", "c1", "c2", "c3")))
            .ids();
        let m = Mat4::identity();
        let a = Texture::new(1, 0.0, 0.5, 0.0, 0.25);
        let b = Texture::new(2, 0.5, 1.0, 0.0, 1.0);
        let c = Texture::new(3, 0.25, 0.75, 0.5, 1.0);
        assert_eq!(
            engine_uniforms(&names, m, m, m, m, &slots([a, b, c])),
            named([
                ("a0", UniformValue::Float(0.0)),
                ("a1", UniformValue::Float(0.5)),
                ("a2", UniformValue::Float(0.0)),
                ("a3", UniformValue::Float(0.25)),
                ("c0", UniformValue::Float(0.25)),
                ("c1", UniformValue::Float(0.75)),
                ("c2", UniformValue::Float(0.5)),
                ("c3", UniformValue::Float(1.0)),
            ])
        );
    }

    #[test]
    fn engine_uniforms_skip_unnamed_values_and_missing_texture() {
        let names = ShaderUniformNames {
            view: Some("v".to_string()),
            ..ShaderUniformNames::default()
        }
        .with_texture_sampler("t", Some(TextureRectNames::new("l", "right", "t", "b")))
        .ids();
        let m = Mat4::identity();
        assert_eq!(
            engine_uniforms(&names, m, m, m, m, &slots([Texture::full(0)])),
            named([
                ("v", UniformValue::Mat4(m)),
                ("l", UniformValue::Float(0.0)),
                ("right", UniformValue::Float(1.0)),
                ("t", UniformValue::Float(0.0)),
                ("b", UniformValue::Float(1.0)),
            ])
        );
        // Without a texture there is no rectangle; packing reports the uniforms as missing.
        assert_eq!(
            engine_uniforms(&names, m, m, m, m, &EMPTY),
            named([("v", UniformValue::Mat4(m))])
        );
    }

    /// Ids of the basic test shader's names: its one sampler samples slot 0.
    fn basic_ids() -> ShaderUniformIds {
        basic_names().ids()
    }

    #[test]
    fn samplers_get_the_material_texture_atlas() {
        let layout = ShaderLayout::reflect(VERT, FRAG).unwrap();
        let texture = Texture::new(8, 0.5, 1.0, 0.0, 0.25);
        let mut issues = Vec::new();

        let sources = sampler_sources(
            &layout,
            &basic_ids(),
            &slots([texture]),
            |a| a == 8,
            &mut issues,
        );

        assert_eq!(issues, []);
        assert_eq!(sources, [SamplerSource::Atlas(Some(8))]);
    }

    #[test]
    fn each_texture_sampler_samples_its_slot() {
        let layout = ShaderLayout::reflect(VERT, TWO_TEXTURES_FRAG).unwrap();
        let names = ShaderUniformNames::default()
            .with_texture_sampler("colorMap", None)
            .with_texture_sampler("normalMap", None)
            .ids();
        let mut issues = Vec::new();
        let sources = sampler_sources(
            &layout,
            &names,
            &slots([Texture::full(5), Texture::full(6)]),
            |_| true,
            &mut issues,
        );
        assert_eq!(issues, []);
        // In `layout.samplers` order (by binding): colorMap, normalMap.
        let atlas_of = |name: &str| {
            let i = layout.samplers.iter().position(|s| s.name == name).unwrap();
            sources[i]
        };
        assert_eq!(atlas_of("colorMap"), SamplerSource::Atlas(Some(5)));
        assert_eq!(atlas_of("normalMap"), SamplerSource::Atlas(Some(6)));

        // Slots swapped in the names: the samplers follow.
        let swapped = ShaderUniformNames::default()
            .with_texture_sampler("normalMap", None)
            .with_texture_sampler("colorMap", None)
            .ids();
        let sources = sampler_sources(
            &layout,
            &swapped,
            &slots([Texture::full(5), Texture::full(6)]),
            |_| true,
            &mut issues,
        );
        let i = layout
            .samplers
            .iter()
            .position(|s| s.name == "colorMap")
            .unwrap();
        assert_eq!(sources[i], SamplerSource::Atlas(Some(6)));
    }

    #[test]
    fn missing_texture_or_atlas_is_reported() {
        let layout = ShaderLayout::reflect(VERT, FRAG).unwrap();
        let mut issues = Vec::new();
        assert_eq!(
            sampler_sources(&layout, &basic_ids(), &EMPTY, |_| true, &mut issues),
            [SamplerSource::Atlas(None)]
        );
        assert_eq!(issues, [UniformIssue::NoTexture(0)]);

        issues.clear();
        assert_eq!(
            sampler_sources(
                &layout,
                &basic_ids(),
                &slots([Texture::full(99)]),
                |_| false,
                &mut issues
            ),
            [SamplerSource::Atlas(None)]
        );
        assert_eq!(issues, [UniformIssue::UnknownAtlas { slot: 0, atlas: 99 }]);
    }

    #[test]
    fn an_empty_second_slot_is_reported_with_its_number() {
        let layout = ShaderLayout::reflect(VERT, TWO_TEXTURES_FRAG).unwrap();
        let names = ShaderUniformNames::default()
            .with_texture_sampler("colorMap", None)
            .with_texture_sampler("normalMap", None)
            .ids();
        let mut issues = Vec::new();
        sampler_sources(
            &layout,
            &names,
            &slots([Texture::full(1)]),
            |_| true,
            &mut issues,
        );
        assert_eq!(issues, [UniformIssue::NoTexture(1)]);
    }

    #[test]
    fn shaders_without_samplers_need_no_texture() {
        let layout = ShaderLayout::default();
        let mut issues = Vec::new();
        let names = ShaderUniformIds::default();
        assert!(sampler_sources(&layout, &names, &EMPTY, |_| true, &mut issues).is_empty());
        assert_eq!(issues, []);
    }

    #[test]
    fn the_shadow_map_sampler_samples_the_shadow_map() {
        let layout = ShaderLayout::reflect(VERT, FRAG).unwrap();
        let names = ShaderUniformIds {
            shadow_map: Some(layout.samplers[0].name),
            ..ShaderUniformIds::default()
        };
        let mut issues = Vec::new();
        // The shader's only sampler is the shadow map: no texture needed.
        assert_eq!(
            sampler_sources(&layout, &names, &EMPTY, |_| true, &mut issues),
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
