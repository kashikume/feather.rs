//! What a shader expects from the engine: its uniform blocks, push constants and samplers,
//! read from the shader's SPIR-V (reflection).

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Context, Result};
use spirq::prelude::*;
use spirq::spirv::{Dim, ExecutionModel};
use spirq::ty::{MatrixAxisOrder, ScalarType, Type};

use super::uniform::UniformValue;

/// Type of a uniform as declared in a shader. Each variant matches a `UniformValue` variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UniformType {
    Float,
    Int,
    /// Also used for GLSL `bool`, which is stored as a 32-bit integer in uniform blocks.
    UInt,
    Vec2,
    Vec3,
    Vec4,
    IVec2,
    IVec3,
    IVec4,
    UVec2,
    UVec3,
    UVec4,
    Mat2,
    Mat3,
    Mat4,
}

impl UniformType {
    /// Whether `value` can be written to a uniform of this type.
    pub fn accepts(&self, value: &UniformValue) -> bool {
        matches!(
            (self, value),
            (UniformType::Float, UniformValue::Float(_))
                | (UniformType::Int, UniformValue::Int(_))
                | (UniformType::UInt, UniformValue::UInt(_))
                | (UniformType::UInt, UniformValue::Bool(_))
                | (UniformType::Vec2, UniformValue::Vec2(_))
                | (UniformType::Vec3, UniformValue::Vec3(_))
                | (UniformType::Vec4, UniformValue::Vec4(_))
                | (UniformType::IVec2, UniformValue::IVec2(_))
                | (UniformType::IVec3, UniformValue::IVec3(_))
                | (UniformType::IVec4, UniformValue::IVec4(_))
                | (UniformType::UVec2, UniformValue::UVec2(_))
                | (UniformType::UVec3, UniformValue::UVec3(_))
                | (UniformType::UVec4, UniformValue::UVec4(_))
                | (UniformType::Mat2, UniformValue::Mat2(_))
                | (UniformType::Mat3, UniformValue::Mat3(_))
                | (UniformType::Mat4, UniformValue::Mat4(_))
        )
    }

    fn from_spirv(ty: &Type) -> Option<Self> {
        use UniformType::*;
        let scalar = |s: &ScalarType| match s {
            ScalarType::Float { bits: 32 } => Some(0),
            ScalarType::Integer {
                bits: 32,
                is_signed: true,
            } => Some(1),
            ScalarType::Integer {
                bits: 32,
                is_signed: false,
            } => Some(2),
            _ => None,
        };
        match ty {
            Type::Scalar(s) => [Float, Int, UInt].get(scalar(s)?).copied(),
            Type::Vector(v) => {
                let kind = scalar(&v.scalar_ty)?;
                let table = [
                    [Vec2, Vec3, Vec4],
                    [IVec2, IVec3, IVec4],
                    [UVec2, UVec3, UVec4],
                ];
                table[kind].get((v.nscalar as usize).checked_sub(2)?).copied()
            }
            Type::Matrix(m) => {
                if scalar(&m.vector_ty.scalar_ty)? != 0
                    || m.axis_order == Some(MatrixAxisOrder::RowMajor)
                    || m.nvector != m.vector_ty.nscalar
                {
                    return None;
                }
                [Mat2, Mat3, Mat4].get((m.nvector as usize).checked_sub(2)?).copied()
            }
            _ => None,
        }
    }
}

/// Shader stages that use a resource.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShaderStages {
    pub vertex: bool,
    pub fragment: bool,
}

impl ShaderStages {
    pub const VERTEX: Self = Self {
        vertex: true,
        fragment: false,
    };
    pub const FRAGMENT: Self = Self {
        vertex: false,
        fragment: true,
    };

    fn union(self, other: Self) -> Self {
        Self {
            vertex: self.vertex || other.vertex,
            fragment: self.fragment || other.fragment,
        }
    }
}

/// A member of a uniform block or of the push constant block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniformMember {
    pub name: String,
    /// Byte offset in the block.
    pub offset: u32,
    pub ty: UniformType,
    /// Bytes between matrix columns (16 for every matrix in std140). `None` for non-matrices.
    pub matrix_stride: Option<u32>,
}

/// A uniform buffer (`layout(set = S, binding = B) uniform Block { ... }`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniformBlock {
    pub set: u32,
    pub binding: u32,
    /// Size of the block in bytes.
    pub size: u32,
    pub stages: ShaderStages,
    pub members: Vec<UniformMember>,
}

/// The push constant block (`layout(push_constant) uniform Block { ... }`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushConstantBlock {
    /// Size of the block in bytes.
    pub size: u32,
    pub stages: ShaderStages,
    pub members: Vec<UniformMember>,
}

/// A combined image sampler (`layout(set = S, binding = B) uniform sampler2D name;`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SamplerBinding {
    pub name: String,
    pub set: u32,
    pub binding: u32,
    pub stages: ShaderStages,
}

/// Where a named uniform goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniformLocation<'a> {
    Block {
        block: &'a UniformBlock,
        member: &'a UniformMember,
    },
    PushConstant(&'a UniformMember),
    Sampler(&'a SamplerBinding),
}

/// Resources a shader (vertex + fragment stage) uses, read from its SPIR-V.
///
/// Uniform names are the GLSL names of block members and samplers; the block names
/// themselves don't matter. Names must be unique across the whole shader. Supported:
/// uniform blocks with members of the types in `UniformType`, one push constant block,
/// and `sampler2D` samplers. Anything else (arrays, nested structs, storage buffers,
/// row-major matrices, ...) makes reflection fail.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShaderLayout {
    /// Sorted by (set, binding).
    pub uniform_blocks: Vec<UniformBlock>,
    pub push_constants: Option<PushConstantBlock>,
    /// Sorted by (set, binding).
    pub samplers: Vec<SamplerBinding>,
}

impl ShaderLayout {
    /// Reflects the vertex and fragment stages and merges their resources. A resource used by
    /// both stages appears once, with both stages set.
    pub fn reflect(vertex: &[u8], fragment: &[u8]) -> Result<Self> {
        let mut builder = LayoutBuilder::default();
        builder
            .add_stage(vertex, ExecutionModel::Vertex, ShaderStages::VERTEX)
            .context("vertex shader")?;
        builder
            .add_stage(fragment, ExecutionModel::Fragment, ShaderStages::FRAGMENT)
            .context("fragment shader")?;
        let layout = builder.build();
        layout.check_unique_names()?;
        Ok(layout)
    }

    /// Finds where the uniform `name` goes.
    pub fn find(&self, name: &str) -> Option<UniformLocation<'_>> {
        for block in &self.uniform_blocks {
            if let Some(member) = block.members.iter().find(|m| m.name == name) {
                return Some(UniformLocation::Block { block, member });
            }
        }
        if let Some(push) = &self.push_constants {
            if let Some(member) = push.members.iter().find(|m| m.name == name) {
                return Some(UniformLocation::PushConstant(member));
            }
        }
        self.samplers
            .iter()
            .find(|s| s.name == name)
            .map(UniformLocation::Sampler)
    }

    /// Number of descriptor sets the shader uses (highest set index + 1).
    pub fn set_count(&self) -> u32 {
        let blocks = self.uniform_blocks.iter().map(|b| b.set);
        let samplers = self.samplers.iter().map(|s| s.set);
        blocks.chain(samplers).max().map_or(0, |s| s + 1)
    }

    fn check_unique_names(&self) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        let block_names = self.uniform_blocks.iter().flat_map(|b| &b.members);
        let push_names = self.push_constants.iter().flat_map(|p| &p.members);
        let names = block_names
            .chain(push_names)
            .map(|m| &m.name)
            .chain(self.samplers.iter().map(|s| &s.name));
        for name in names {
            if !seen.insert(name) {
                bail!("uniform name `{name}` is used more than once");
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct LayoutBuilder {
    blocks: BTreeMap<(u32, u32), UniformBlock>,
    samplers: BTreeMap<(u32, u32), SamplerBinding>,
    push_constants: Option<PushConstantBlock>,
}

impl LayoutBuilder {
    fn add_stage(&mut self, spirv: &[u8], model: ExecutionModel, stage: ShaderStages) -> Result<()> {
        let entry_points = ReflectConfig::new()
            .spv(spirv)
            .ref_all_rscs(true)
            .combine_img_samplers(true)
            .reflect()
            .map_err(|e| anyhow!("SPIR-V reflection failed: {e}"))?;
        let entry_point = entry_points
            .iter()
            .find(|e| e.exec_model == model)
            .ok_or_else(|| anyhow!("no {model:?} entry point"))?;

        for var in &entry_point.vars {
            match var {
                Variable::Descriptor {
                    name,
                    desc_bind,
                    desc_ty,
                    ty,
                    nbind,
                } => {
                    let (set, binding) = (desc_bind.set(), desc_bind.bind());
                    let what = || format!("{} (set {set}, binding {binding})", display(name));
                    if *nbind != 1 {
                        bail!("{}: descriptor arrays are not supported", what());
                    }
                    match desc_ty {
                        DescriptorType::UniformBuffer() => {
                            let (size, members) =
                                struct_members(ty).with_context(|| what())?;
                            let block = UniformBlock {
                                set,
                                binding,
                                size,
                                stages: stage,
                                members,
                            };
                            merge_block(&mut self.blocks, block).with_context(|| what())?;
                        }
                        DescriptorType::CombinedImageSampler() => {
                            let is_2d = matches!(
                                ty,
                                Type::CombinedImageSampler(t)
                                    if t.sampled_image_ty.dim == Dim::Dim2D
                                        && !t.sampled_image_ty.is_array
                            );
                            if !is_2d {
                                bail!("{}: only sampler2D is supported", what());
                            }
                            let name = name.clone().ok_or_else(|| {
                                anyhow!("{}: sampler has no name (debug info stripped?)", what())
                            })?;
                            let sampler = SamplerBinding {
                                name,
                                set,
                                binding,
                                stages: stage,
                            };
                            merge_sampler(&mut self.samplers, sampler)?;
                        }
                        other => bail!("{}: unsupported descriptor type {other:?}", what()),
                    }
                }
                Variable::PushConstant { ty, .. } => {
                    let (size, members) = struct_members(ty).context("push constant block")?;
                    let push = PushConstantBlock {
                        size,
                        stages: stage,
                        members,
                    };
                    self.push_constants = Some(match self.push_constants.take() {
                        None => push,
                        Some(existing) => merge_push(existing, push)?,
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn build(self) -> ShaderLayout {
        ShaderLayout {
            uniform_blocks: self.blocks.into_values().collect(),
            push_constants: self.push_constants,
            samplers: self.samplers.into_values().collect(),
        }
    }
}

fn display(name: &Option<String>) -> &str {
    name.as_deref().unwrap_or("<unnamed>")
}

/// Size and members of a block type (uniform block or push constants).
fn struct_members(ty: &Type) -> Result<(u32, Vec<UniformMember>)> {
    let Type::Struct(s) = ty else {
        bail!("expected a block, found {ty:?}");
    };
    let size = ty
        .nbyte()
        .ok_or_else(|| anyhow!("block has no fixed size"))? as u32;
    let members = s
        .members
        .iter()
        .map(|m| {
            let name = m
                .name
                .clone()
                .ok_or_else(|| anyhow!("block member has no name (debug info stripped?)"))?;
            let offset = m
                .offset
                .ok_or_else(|| anyhow!("member `{name}` has no offset"))? as u32;
            let ty_ = UniformType::from_spirv(&m.ty)
                .ok_or_else(|| anyhow!("member `{name}` has an unsupported type {:?}", m.ty))?;
            let matrix_stride = match &m.ty {
                Type::Matrix(mat) => Some(
                    mat.stride
                        .ok_or_else(|| anyhow!("matrix `{name}` has no stride"))?
                        as u32,
                ),
                _ => None,
            };
            Ok(UniformMember {
                name,
                offset,
                ty: ty_,
                matrix_stride,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((size, members))
}

fn merge_block(blocks: &mut BTreeMap<(u32, u32), UniformBlock>, block: UniformBlock) -> Result<()> {
    let key = (block.set, block.binding);
    match blocks.get_mut(&key) {
        None => {
            blocks.insert(key, block);
        }
        Some(existing) => {
            if existing.members != block.members || existing.size != block.size {
                bail!("the stages declare different blocks at the same binding");
            }
            existing.stages = existing.stages.union(block.stages);
        }
    }
    Ok(())
}

fn merge_sampler(
    samplers: &mut BTreeMap<(u32, u32), SamplerBinding>,
    sampler: SamplerBinding,
) -> Result<()> {
    let key = (sampler.set, sampler.binding);
    match samplers.get_mut(&key) {
        None => {
            samplers.insert(key, sampler);
        }
        Some(existing) => {
            if existing.name != sampler.name {
                bail!(
                    "samplers `{}` and `{}` share set {} binding {}",
                    existing.name,
                    sampler.name,
                    key.0,
                    key.1
                );
            }
            existing.stages = existing.stages.union(sampler.stages);
        }
    }
    Ok(())
}

/// Push constants of two stages: members with the same name must match; the block covers both.
fn merge_push(a: PushConstantBlock, b: PushConstantBlock) -> Result<PushConstantBlock> {
    let mut members = a.members;
    for member in b.members {
        match members.iter().find(|m| m.name == member.name) {
            Some(existing) if *existing != member => {
                bail!("push constant `{}` differs between stages", member.name)
            }
            Some(_) => {}
            None => members.push(member),
        }
    }
    members.sort_by_key(|m| m.offset);
    Ok(PushConstantBlock {
        size: a.size.max(b.size),
        stages: a.stages.union(b.stages),
        members,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feather::shader::tests::{FRAG, VERT};

    fn mat4(name: &str, offset: u32) -> UniformMember {
        UniformMember {
            name: name.to_string(),
            offset,
            ty: UniformType::Mat4,
            matrix_stride: Some(16),
        }
    }

    #[test]
    fn reflects_basic_shader() {
        let layout = ShaderLayout::reflect(VERT, FRAG).unwrap();

        assert_eq!(
            layout.uniform_blocks,
            [UniformBlock {
                set: 0,
                binding: 0,
                size: 128,
                stages: ShaderStages::VERTEX,
                members: vec![mat4("view", 0), mat4("proj", 64)],
            }]
        );
        assert_eq!(
            layout.push_constants,
            Some(PushConstantBlock {
                size: 64,
                stages: ShaderStages::VERTEX,
                members: vec![mat4("model", 0)],
            })
        );
        assert_eq!(
            layout.samplers,
            [SamplerBinding {
                name: "texSampler".to_string(),
                set: 0,
                binding: 1,
                stages: ShaderStages::FRAGMENT,
            }]
        );
        assert_eq!(layout.set_count(), 1);
    }

    #[test]
    fn find_uniforms_by_name() {
        let layout = ShaderLayout::reflect(VERT, FRAG).unwrap();

        match layout.find("proj") {
            Some(UniformLocation::Block { block, member }) => {
                assert_eq!((block.set, block.binding, member.offset), (0, 0, 64));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            layout.find("model"),
            Some(UniformLocation::PushConstant(m)) if m.offset == 0
        ));
        assert!(matches!(
            layout.find("texSampler"),
            Some(UniformLocation::Sampler(s)) if s.binding == 1
        ));
        assert_eq!(layout.find("missing"), None);
    }

    #[test]
    fn stages_must_be_the_right_kind() {
        // Fragment code passed as the vertex stage has no vertex entry point.
        assert!(ShaderLayout::reflect(FRAG, FRAG).is_err());
    }

    #[test]
    fn uniform_type_accepts_matching_values() {
        use crate::feather::math::{Mat4, Vec3};
        use cgmath::SquareMatrix;

        assert!(UniformType::Float.accepts(&UniformValue::Float(1.0)));
        assert!(UniformType::Vec3.accepts(&UniformValue::Vec3(Vec3::new(0.0, 0.0, 0.0))));
        assert!(UniformType::Mat4.accepts(&UniformValue::Mat4(Mat4::identity())));
        assert!(UniformType::UInt.accepts(&UniformValue::Bool(true)));
        assert!(!UniformType::Float.accepts(&UniformValue::Int(1)));
        assert!(!UniformType::Mat4.accepts(&UniformValue::Texture(0)));
    }

    const TYPES_VERT: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/shaders/tests/types.vert.spv"));
    const TYPES_FRAG: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/shaders/tests/types.frag.spv"));
    const ARRAY_FRAG: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/shaders/tests/array.frag.spv"));
    const DUPLICATE_FRAG: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/shaders/tests/duplicate.frag.spv"));

    fn member(name: &str, offset: u32, ty: UniformType) -> UniformMember {
        let matrix_stride = matches!(ty, UniformType::Mat2 | UniformType::Mat3 | UniformType::Mat4)
            .then_some(16);
        UniformMember {
            name: name.to_string(),
            offset,
            ty,
            matrix_stride,
        }
    }

    #[test]
    fn reflects_every_uniform_type_with_std140_offsets() {
        use UniformType::*;

        let layout = ShaderLayout::reflect(TYPES_VERT, TYPES_FRAG).unwrap();
        let values = layout
            .uniform_blocks
            .iter()
            .find(|b| (b.set, b.binding) == (1, 0))
            .unwrap();

        assert_eq!(
            values.members,
            [
                member("f", 0, Float),
                member("i", 4, Int),
                member("u", 8, UInt),
                member("b", 12, UInt), // bool is stored as uint
                member("v2", 16, Vec2),
                member("v3", 32, Vec3),
                member("v4", 48, Vec4),
                member("i2", 64, IVec2),
                member("i3", 80, IVec3),
                member("i4", 96, IVec4),
                member("u2", 112, UVec2),
                member("u3", 128, UVec3),
                member("u4", 144, UVec4),
                member("m2", 160, Mat2),
                member("m3", 192, Mat3),
                member("m4", 240, Mat4),
            ]
        );
        assert_eq!(values.size, 304);
        assert_eq!(values.stages, ShaderStages::VERTEX);
        assert_eq!(layout.set_count(), 2);
    }

    #[test]
    fn resources_used_by_both_stages_are_merged() {
        let layout = ShaderLayout::reflect(TYPES_VERT, TYPES_FRAG).unwrap();
        let both = ShaderStages {
            vertex: true,
            fragment: true,
        };

        let shared = &layout.uniform_blocks[0];
        assert_eq!((shared.set, shared.binding), (0, 0));
        assert_eq!(shared.stages, both);
        assert_eq!(layout.uniform_blocks.len(), 2);

        let push = layout.push_constants.as_ref().unwrap();
        assert_eq!(push.stages, both);
        assert_eq!(push.size, 80);
        assert_eq!(
            push.members,
            [member("model", 0, UniformType::Mat4), member("tint", 64, UniformType::Vec4)]
        );

        assert_eq!(
            layout.samplers,
            [SamplerBinding {
                name: "albedo".to_string(),
                set: 1,
                binding: 1,
                stages: ShaderStages::FRAGMENT,
            }]
        );
    }

    #[test]
    fn arrays_are_not_supported() {
        let err = ShaderLayout::reflect(VERT, ARRAY_FRAG).unwrap_err();
        assert!(format!("{err:#}").contains("colors"), "{err:#}");
    }

    #[test]
    fn duplicate_uniform_names_fail() {
        let err = ShaderLayout::reflect(VERT, DUPLICATE_FRAG).unwrap_err();
        assert!(format!("{err:#}").contains("`view`"), "{err:#}");
    }

    #[test]
    fn set_count_of_empty_layout_is_zero() {
        assert_eq!(ShaderLayout::default().set_count(), 0);
    }
}
