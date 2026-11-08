//! Packing uniform values into the bytes a shader reads: one aligned slice per uniform block in
//! a per-frame buffer, plus the push constant bytes.

use super::shaderlayout::{ShaderLayout, UniformMember, UniformType};
use super::uniform::UniformValue;
use super::uniforms::Uniforms;

/// The values available to one draw. `engine` values (the model, view and projection
/// matrices under the shader's names) take precedence over the node's dynamic uniforms.
pub struct UniformValues<'a> {
    pub engine: &'a [(&'a str, UniformValue)],
    pub node: &'a Uniforms,
}

impl UniformValues<'_> {
    pub fn get(&self, name: &str) -> Option<&UniformValue> {
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
        match values.get(&member.name) {
            None => issues.push(UniformIssue::Missing(member.name.clone())),
            Some(value) if !member.ty.accepts(value) => issues.push(UniformIssue::WrongType {
                name: member.name.clone(),
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

/// Node uniforms the shader has no place for.
pub fn unused_uniforms(layout: &ShaderLayout, uniforms: &Uniforms) -> Vec<UniformIssue> {
    uniforms
        .iter()
        .filter(|u| layout.find(&u.name).is_none())
        .map(|u| UniformIssue::Unused(u.name.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feather::math::{Mat4, Vec4};
    use crate::feather::shader::tests::{FRAG, VERT};
    use cgmath::SquareMatrix;

    fn mat_bytes(m: Mat4) -> Vec<u8> {
        let mut buf = vec![0; 64];
        UniformValue::Mat4(m).write_bytes(&mut buf, 0, 16);
        buf
    }

    fn translation(x: f32) -> Mat4 {
        Mat4::from_translation(crate::feather::math::Vec3::new(x, 0.0, 0.0))
    }

    #[test]
    fn engine_values_override_node_values() {
        let mut node = Uniforms::new();
        node.set("view", Mat4::identity()).set("tint", 1.0f32);
        let engine = [("view", UniformValue::Mat4(translation(1.0)))];
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
        let engine = [
            ("model", UniformValue::Mat4(translation(1.0))),
            ("view", UniformValue::Mat4(translation(2.0))),
            ("proj", UniformValue::Mat4(translation(3.0))),
        ];
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
    fn missing_wrong_and_unused_uniforms_are_reported() {
        let layout = ShaderLayout::reflect(VERT, FRAG).unwrap();
        let mut node = Uniforms::new();
        node.set("view", 1.0f32) // wrong type
            .set("extra", Vec4::new(0.0, 0.0, 0.0, 0.0)) // not in the shader
            .set("texSampler", UniformValue::Texture(0)); // sampler: fine
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
