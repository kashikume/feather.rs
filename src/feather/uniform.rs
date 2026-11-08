use super::math::{IVec2, IVec3, IVec4, Mat2, Mat3, Mat4, UVec2, UVec3, UVec4, Vec2, Vec3, Vec4};

/// A value of any type that can be passed to a shader as a uniform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UniformValue {
    Float(f32),
    Int(i32),
    UInt(u32),
    Bool(bool),
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
    IVec2(IVec2),
    IVec3(IVec3),
    IVec4(IVec4),
    UVec2(UVec2),
    UVec3(UVec3),
    UVec4(UVec4),
    Mat2(Mat2),
    Mat3(Mat3),
    Mat4(Mat4),
    /// Handle of a texture in `Scene::textures`, for `sampler2D` uniforms.
    Texture(usize),
}

impl UniformValue {
    /// The matching GLSL type name, e.g. `vec3` or `sampler2D`.
    pub fn glsl_type(&self) -> &'static str {
        match self {
            UniformValue::Float(_) => "float",
            UniformValue::Int(_) => "int",
            UniformValue::UInt(_) => "uint",
            UniformValue::Bool(_) => "bool",
            UniformValue::Vec2(_) => "vec2",
            UniformValue::Vec3(_) => "vec3",
            UniformValue::Vec4(_) => "vec4",
            UniformValue::IVec2(_) => "ivec2",
            UniformValue::IVec3(_) => "ivec3",
            UniformValue::IVec4(_) => "ivec4",
            UniformValue::UVec2(_) => "uvec2",
            UniformValue::UVec3(_) => "uvec3",
            UniformValue::UVec4(_) => "uvec4",
            UniformValue::Mat2(_) => "mat2",
            UniformValue::Mat3(_) => "mat3",
            UniformValue::Mat4(_) => "mat4",
            UniformValue::Texture(_) => "sampler2D",
        }
    }
}

macro_rules! impl_from_for_uniform_value {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl From<$ty> for UniformValue {
                fn from(value: $ty) -> Self {
                    UniformValue::$variant(value)
                }
            }
        )*
    };
}

// No `From<usize>`: a texture handle must be passed explicitly as `UniformValue::Texture`.
impl_from_for_uniform_value!(
    f32 => Float,
    i32 => Int,
    u32 => UInt,
    bool => Bool,
    Vec2 => Vec2,
    Vec3 => Vec3,
    Vec4 => Vec4,
    IVec2 => IVec2,
    IVec3 => IVec3,
    IVec4 => IVec4,
    UVec2 => UVec2,
    UVec3 => UVec3,
    UVec4 => UVec4,
    Mat2 => Mat2,
    Mat3 => Mat3,
    Mat4 => Mat4,
);

/// A named uniform value.
#[derive(Clone, Debug, PartialEq)]
pub struct Uniform {
    pub name: String,
    pub value: UniformValue,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cgmath::SquareMatrix;

    #[test]
    fn from_converts_to_matching_variant() {
        assert_eq!(UniformValue::from(1.5f32), UniformValue::Float(1.5));
        assert_eq!(UniformValue::from(-3i32), UniformValue::Int(-3));
        assert_eq!(UniformValue::from(3u32), UniformValue::UInt(3));
        assert_eq!(UniformValue::from(true), UniformValue::Bool(true));
        assert_eq!(
            UniformValue::from(Vec3::new(1.0, 2.0, 3.0)),
            UniformValue::Vec3(Vec3::new(1.0, 2.0, 3.0))
        );
        assert_eq!(
            UniformValue::from(IVec2::new(1, 2)),
            UniformValue::IVec2(IVec2::new(1, 2))
        );
        assert_eq!(
            UniformValue::from(UVec4::new(1, 2, 3, 4)),
            UniformValue::UVec4(UVec4::new(1, 2, 3, 4))
        );
        assert_eq!(
            UniformValue::from(Mat4::identity()),
            UniformValue::Mat4(Mat4::identity())
        );
    }

    #[test]
    fn glsl_type_names() {
        let cases = [
            (UniformValue::Float(0.0), "float"),
            (UniformValue::Int(0), "int"),
            (UniformValue::UInt(0), "uint"),
            (UniformValue::Bool(false), "bool"),
            (UniformValue::Vec2(Vec2::new(0.0, 0.0)), "vec2"),
            (UniformValue::Vec3(Vec3::new(0.0, 0.0, 0.0)), "vec3"),
            (UniformValue::Vec4(Vec4::new(0.0, 0.0, 0.0, 0.0)), "vec4"),
            (UniformValue::IVec2(IVec2::new(0, 0)), "ivec2"),
            (UniformValue::IVec3(IVec3::new(0, 0, 0)), "ivec3"),
            (UniformValue::IVec4(IVec4::new(0, 0, 0, 0)), "ivec4"),
            (UniformValue::UVec2(UVec2::new(0, 0)), "uvec2"),
            (UniformValue::UVec3(UVec3::new(0, 0, 0)), "uvec3"),
            (UniformValue::UVec4(UVec4::new(0, 0, 0, 0)), "uvec4"),
            (UniformValue::Mat2(Mat2::identity()), "mat2"),
            (UniformValue::Mat3(Mat3::identity()), "mat3"),
            (UniformValue::Mat4(Mat4::identity()), "mat4"),
            (UniformValue::Texture(0), "sampler2D"),
        ];
        for (value, expected) in cases {
            assert_eq!(value.glsl_type(), expected);
        }
    }
}
