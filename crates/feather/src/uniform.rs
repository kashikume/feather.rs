use super::math::{IVec2, IVec3, IVec4, Mat2, Mat3, Mat4, UVec2, UVec3, UVec4, Vec2, Vec3, Vec4};
use super::uniformname::UniformName;

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
}

impl UniformValue {
    /// The matching GLSL type name, e.g. `vec3`.
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
        }
    }

    /// Writes the value into `buf` at `offset` the way uniform blocks and push constants
    /// store it: 4-byte scalars (`bool` as a `uint` 0 or 1), vector components packed, and
    /// matrix column `c` at `offset + c * matrix_stride` (16 for every matrix in std140).
    ///
    /// Returns `false` and writes nothing when the value doesn't fit in `buf`.
    pub fn write_bytes(&self, buf: &mut [u8], offset: usize, matrix_stride: usize) -> bool {
        let f = |v: f32| v.to_bits();
        let i = |v: i32| v as u32;
        let columns: Vec<Vec<u32>> = match *self {
            UniformValue::Float(v) => vec![vec![f(v)]],
            UniformValue::Int(v) => vec![vec![i(v)]],
            UniformValue::UInt(v) => vec![vec![v]],
            UniformValue::Bool(v) => vec![vec![v as u32]],
            UniformValue::Vec2(v) => vec![vec![f(v.x), f(v.y)]],
            UniformValue::Vec3(v) => vec![vec![f(v.x), f(v.y), f(v.z)]],
            UniformValue::Vec4(v) => vec![vec![f(v.x), f(v.y), f(v.z), f(v.w)]],
            UniformValue::IVec2(v) => vec![vec![i(v.x), i(v.y)]],
            UniformValue::IVec3(v) => vec![vec![i(v.x), i(v.y), i(v.z)]],
            UniformValue::IVec4(v) => vec![vec![i(v.x), i(v.y), i(v.z), i(v.w)]],
            UniformValue::UVec2(v) => vec![vec![v.x, v.y]],
            UniformValue::UVec3(v) => vec![vec![v.x, v.y, v.z]],
            UniformValue::UVec4(v) => vec![vec![v.x, v.y, v.z, v.w]],
            UniformValue::Mat2(m) => [m.x, m.y].iter().map(|c| vec![f(c.x), f(c.y)]).collect(),
            UniformValue::Mat3(m) => [m.x, m.y, m.z]
                .iter()
                .map(|c| vec![f(c.x), f(c.y), f(c.z)])
                .collect(),
            UniformValue::Mat4(m) => [m.x, m.y, m.z, m.w]
                .iter()
                .map(|c| vec![f(c.x), f(c.y), f(c.z), f(c.w)])
                .collect(),
        };

        let column_offset = |c: usize| offset + c * matrix_stride;
        let end = column_offset(columns.len() - 1) + columns.last().unwrap().len() * 4;
        if end > buf.len() {
            return false;
        }
        for (c, column) in columns.iter().enumerate() {
            for (k, word) in column.iter().enumerate() {
                let at = column_offset(c) + k * 4;
                buf[at..at + 4].copy_from_slice(&word.to_ne_bytes());
            }
        }
        true
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Uniform {
    pub name: UniformName,
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

    /// Reads the 4-byte words of `buf`.
    fn words(buf: &[u8]) -> Vec<u32> {
        buf.chunks_exact(4)
            .map(|w| u32::from_ne_bytes(w.try_into().unwrap()))
            .collect()
    }

    fn written(value: UniformValue, len: usize, offset: usize, stride: usize) -> Vec<u32> {
        let mut buf = vec![0xFF; len];
        assert!(value.write_bytes(&mut buf, offset, stride));
        words(&buf)
    }

    const X: u32 = 0xFFFF_FFFF; // untouched

    #[test]
    fn write_scalars() {
        assert_eq!(
            written(UniformValue::Float(1.5), 8, 4, 16),
            [X, 1.5f32.to_bits()]
        );
        assert_eq!(written(UniformValue::Int(-2), 4, 0, 16), [(-2i32) as u32]);
        assert_eq!(written(UniformValue::UInt(7), 4, 0, 16), [7]);
        assert_eq!(written(UniformValue::Bool(true), 4, 0, 16), [1]);
        assert_eq!(written(UniformValue::Bool(false), 4, 0, 16), [0]);
    }

    #[test]
    fn write_vectors() {
        let f = |v: f32| v.to_bits();
        assert_eq!(
            written(UniformValue::Vec2(Vec2::new(1.0, 2.0)), 8, 0, 16),
            [f(1.0), f(2.0)]
        );
        assert_eq!(
            written(UniformValue::Vec3(Vec3::new(1.0, 2.0, 3.0)), 16, 0, 16),
            [f(1.0), f(2.0), f(3.0), X]
        );
        assert_eq!(
            written(UniformValue::Vec4(Vec4::new(1.0, 2.0, 3.0, 4.0)), 16, 0, 16),
            [f(1.0), f(2.0), f(3.0), f(4.0)]
        );
        assert_eq!(
            written(UniformValue::IVec2(IVec2::new(-1, 2)), 8, 0, 16),
            [(-1i32) as u32, 2]
        );
        assert_eq!(
            written(UniformValue::IVec3(IVec3::new(1, 2, 3)), 12, 0, 16),
            [1, 2, 3]
        );
        assert_eq!(
            written(UniformValue::IVec4(IVec4::new(1, 2, 3, 4)), 16, 0, 16),
            [1, 2, 3, 4]
        );
        assert_eq!(
            written(UniformValue::UVec2(UVec2::new(5, 6)), 8, 0, 16),
            [5, 6]
        );
        assert_eq!(
            written(UniformValue::UVec3(UVec3::new(5, 6, 7)), 12, 0, 16),
            [5, 6, 7]
        );
        assert_eq!(
            written(UniformValue::UVec4(UVec4::new(5, 6, 7, 8)), 16, 0, 16),
            [5, 6, 7, 8]
        );
    }

    #[test]
    fn write_matrices_column_by_column_with_stride() {
        let f = |v: f32| v.to_bits();
        // Columns (1, 2), (3, 4); std140 pads each column to 16 bytes.
        let m2 = Mat2::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(
            written(UniformValue::Mat2(m2), 32, 0, 16),
            [f(1.0), f(2.0), X, X, f(3.0), f(4.0), X, X]
        );
        // std430 (push constants): mat2 columns are 8 bytes apart.
        assert_eq!(
            written(UniformValue::Mat2(m2), 16, 0, 8),
            [f(1.0), f(2.0), f(3.0), f(4.0)]
        );

        let m3 = Mat3::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        assert_eq!(
            written(UniformValue::Mat3(m3), 48, 0, 16),
            [
                f(1.0),
                f(2.0),
                f(3.0),
                X,
                f(4.0),
                f(5.0),
                f(6.0),
                X,
                f(7.0),
                f(8.0),
                f(9.0),
                X,
            ]
        );

        let m4 = Mat4::from_translation(Vec3::new(5.0, 6.0, 7.0));
        let w = written(UniformValue::Mat4(m4), 64, 0, 16);
        assert_eq!(&w[12..], [f(5.0), f(6.0), f(7.0), f(1.0)]); // last column: translation
        assert_eq!(&w[..4], [f(1.0), 0, 0, 0]);
    }

    #[test]
    fn write_fails_on_overflow() {
        let mut buf = vec![0u8; 16];
        assert!(!UniformValue::Vec4(Vec4::new(1.0, 1.0, 1.0, 1.0)).write_bytes(&mut buf, 4, 16));
        assert!(!UniformValue::Mat2(Mat2::identity()).write_bytes(&mut buf, 0, 16));
        assert_eq!(buf, [0; 16]);
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
        ];
        for (value, expected) in cases {
            assert_eq!(value.glsl_type(), expected);
        }
    }
}
