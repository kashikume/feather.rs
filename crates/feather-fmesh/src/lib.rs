//! Reading and writing `.fmesh` files: feather's binary meshes, made from OBJ files by the
//! `mesh-converter` tool and loaded by the engine's `MeshBuilderFmeshFile`.
//!
//! The format is described in full in `doc/fmesh-format.md`. In short: a fixed 76-byte header
//! (little-endian), then the data: positions, normals, UVs and triangle indices, each as one
//! stream, optionally deflated. Vertex values are stored either as `f32` or as integers `n`
//! with a number of decimals `d` (the value is `n / 10^d`), the integers being written with a
//! fixed width (`i8`, `i16`, `i32`) or as zigzag varints of the differences between
//! consecutive values ("delta").
//!
//! This crate knows nothing of OBJ files nor of Vulkan, so both the engine and build scripts
//! can use it.

mod decode;
mod encode;

pub use decode::{read, read_header, Fmesh};
pub use encode::{write, Written};

/// The first 8 bytes of every `.fmesh` file: `FMESH`, CR, LF, Ctrl-Z. Like PNG's, the line
/// ending bytes reveal a file damaged by a text-mode transfer.
pub const MAGIC: [u8; 8] = *b"FMESH\r\n\x1a";

/// The format version this crate writes, and the only one it reads.
pub const VERSION: u16 = 1;

/// The size of a version 1 header in bytes. A later version may grow the header; the header
/// stores its own size.
pub const HEADER_SIZE: usize = 76;

/// Flag: the data is deflated (raw deflate, RFC 1951).
pub const FLAG_COMPRESSED: u32 = 1;

/// Every flag version 1 knows. Any other bit set makes the file invalid.
pub const KNOWN_FLAGS: u32 = FLAG_COMPRESSED;

/// The most decimals a stream may have.
pub const MAX_DECIMALS: u8 = 18;

/// The largest uncompressed data a reader accepts (1 GiB), so a damaged header can't make it
/// allocate without limit.
pub const MAX_DATA_SIZE: u32 = 1 << 30;

/// Number of values per vertex in the position, normal and UV streams.
pub const POSITION_COMPONENTS: usize = 3;
pub const NORMAL_COMPONENTS: usize = 3;
pub const UV_COMPONENTS: usize = 2;

/// How a stream's values are stored. The codes are the byte in the stream descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Encoding {
    /// `f32` values (vertex streams only).
    F32,
    /// Integers, 1 byte each (vertex streams only).
    I8,
    /// Integers, 2 bytes each (vertex streams only).
    I16,
    /// Integers, 4 bytes each (vertex streams only).
    I32,
    /// Differences between consecutive values, zigzag-encoded as LEB128 varints. In a vertex
    /// stream the difference is to the same component of the previous vertex.
    Delta,
    /// Unsigned 2-byte indices (index stream only).
    U16,
    /// Unsigned 4-byte indices (index stream only).
    U32,
}

impl Encoding {
    /// The code stored in the file.
    pub fn code(self) -> u8 {
        match self {
            Encoding::F32 => 0,
            Encoding::I8 => 1,
            Encoding::I16 => 2,
            Encoding::I32 => 3,
            Encoding::Delta => 4,
            Encoding::U16 => 5,
            Encoding::U32 => 6,
        }
    }

    /// The encoding with the code `code`, if there is one.
    pub fn from_code(code: u8) -> Option<Encoding> {
        Some(match code {
            0 => Encoding::F32,
            1 => Encoding::I8,
            2 => Encoding::I16,
            3 => Encoding::I32,
            4 => Encoding::Delta,
            5 => Encoding::U16,
            6 => Encoding::U32,
            _ => return None,
        })
    }

    /// A short name, for messages and statistics.
    pub fn name(self) -> &'static str {
        match self {
            Encoding::F32 => "f32",
            Encoding::I8 => "i8",
            Encoding::I16 => "i16",
            Encoding::I32 => "i32",
            Encoding::Delta => "delta",
            Encoding::U16 => "u16",
            Encoding::U32 => "u32",
        }
    }

    /// Whether a vertex stream (positions, normals, UVs) may use this encoding.
    pub fn for_vertices(self) -> bool {
        !matches!(self, Encoding::U16 | Encoding::U32)
    }

    /// Whether the index stream may use this encoding.
    pub fn for_indices(self) -> bool {
        matches!(self, Encoding::U16 | Encoding::U32 | Encoding::Delta)
    }

    /// The bytes per value of a fixed-width encoding, `None` for `Delta`.
    pub fn width(self) -> Option<usize> {
        match self {
            Encoding::I8 => Some(1),
            Encoding::I16 | Encoding::U16 => Some(2),
            Encoding::F32 | Encoding::I32 | Encoding::U32 => Some(4),
            Encoding::Delta => None,
        }
    }
}

/// How one stream is stored: 4 bytes in the header (encoding, decimals, 2 reserved zero bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamDescriptor {
    pub encoding: Encoding,
    /// The value of a stored integer `n` is `n / 10^decimals`. Always 0 for `F32` and for
    /// indices.
    pub decimals: u8,
}

/// A file's header.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub version: u16,
    /// The header's size in bytes; the data starts there.
    pub header_size: u16,
    pub flags: u32,
    pub vertex_count: u32,
    /// A multiple of 3: three indices per triangle.
    pub index_count: u32,
    pub positions: StreamDescriptor,
    pub normals: StreamDescriptor,
    pub uvs: StreamDescriptor,
    pub indices: StreamDescriptor,
    /// The smallest x, y, z of the positions (zeros for an empty mesh).
    pub bounds_min: [f32; 3],
    /// The largest x, y, z of the positions (zeros for an empty mesh).
    pub bounds_max: [f32; 3],
    /// The size of the data once decompressed.
    pub data_size: u32,
    /// The size of the data as stored in the file (equal to `data_size` when not compressed).
    pub stored_size: u32,
    /// CRC-32 (IEEE 802.3, as in zip and PNG) of the stored data.
    pub crc32: u32,
}

impl Header {
    pub fn compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }
}

/// The values of one vertex stream, as given to [`write`].
#[derive(Clone, Debug, PartialEq)]
pub enum Stream {
    /// Plain floats, stored as `f32`.
    Float(Vec<f32>),
    /// Integers `n` standing for `n / 10^decimals`.
    Fixed { values: Vec<i32>, decimals: u8 },
}

impl Stream {
    pub fn len(&self) -> usize {
        match self {
            Stream::Float(values) => values.len(),
            Stream::Fixed { values, .. } => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The values as a reader gets them.
    pub fn to_f32(&self) -> Vec<f32> {
        match self {
            Stream::Float(values) => values.clone(),
            Stream::Fixed { values, decimals } => values
                .iter()
                .map(|&n| fixed_to_f32(n as i64, *decimals))
                .collect(),
        }
    }
}

/// A mesh to write: per-vertex streams and triangle indices.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshData {
    /// x, y, z per vertex.
    pub positions: Stream,
    /// x, y, z per vertex.
    pub normals: Stream,
    /// u, v per vertex, V already flipped for Vulkan (0 at the top of the image).
    pub uvs: Stream,
    /// Three per triangle, counter-clockwise seen from the front.
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn vertex_count(&self) -> usize {
        self.positions.len() / POSITION_COMPONENTS
    }
}

/// Which encodings [`write`] uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EncodingChoice {
    /// For each stream, whichever of fixed-width integers and delta is smaller (`F32` for
    /// float streams); `U16` / `U32` or delta for indices, whichever is smaller.
    #[default]
    Auto,
    /// Every vertex stream as `f32`; indices `U16` / `U32`.
    F32,
    /// Fixed-width integers (the narrowest that holds every value); indices `U16` / `U32`.
    Int,
    /// Delta for every integer stream and for indices.
    Delta,
}

/// Whether [`write`] deflates the data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Compression {
    /// Only when it makes the data at least [`AUTO_COMPRESS_MIN_SAVING`] smaller.
    #[default]
    Auto,
    On,
    Off,
}

/// The smallest saving (a fraction of the uncompressed data size) for which
/// [`Compression::Auto`] keeps the data deflated.
pub const AUTO_COMPRESS_MIN_SAVING: f64 = 0.1;

/// Settings of [`write`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WriteOptions {
    pub encoding: EncodingChoice,
    pub compression: Compression,
    /// Deflate level, 0 to 10 (`miniz_oxide`'s scale; 10 is the slowest and smallest).
    pub level: u8,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            encoding: EncodingChoice::Auto,
            compression: Compression::Auto,
            level: 9,
        }
    }
}

/// What is wrong with a file being read, or a mesh being written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FmeshError {
    #[error("not an .fmesh file (wrong magic bytes)")]
    BadMagic,
    #[error(
        "the file is version {found}, this build reads version {supported}: convert the mesh again"
    )]
    UnsupportedVersion { found: u16, supported: u16 },
    #[error("the file is truncated ({0})")]
    Truncated(&'static str),
    #[error("header size {0} is smaller than {HEADER_SIZE} bytes")]
    BadHeaderSize(u16),
    #[error("unknown flags {0:#x}")]
    UnknownFlags(u32),
    #[error("{stream}: invalid encoding {code}")]
    BadEncoding { stream: &'static str, code: u8 },
    #[error("{stream}: invalid decimals {decimals} for this encoding (at most {MAX_DECIMALS}, 0 for f32)")]
    BadDecimals { stream: &'static str, decimals: u8 },
    #[error("data checksum mismatch: header says {expected:#010x}, data has {found:#010x}")]
    Checksum { expected: u32, found: u32 },
    #[error("the data can't be decompressed")]
    Decompress,
    #[error("{0}")]
    Invalid(String),
}

/// Powers of ten from 10^0 to 10^10, each exact in `f32`.
const POW10_F32: [f32; 11] = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10];

/// The float a stored integer `n` with `decimals` decimals stands for: `n / 10^decimals`.
///
/// When `|n| < 2^24` and `decimals <= 10`, both are exact in `f32`, and the division (rounded
/// correctly by IEEE 754) gives the `f32` nearest to the decimal number: the same as parsing
/// its text with `str::parse::<f32>`. Otherwise the division is done in `f64` and rounded to
/// `f32` afterwards, which can (rarely) be one ulp off; the converter checks for that and
/// stores such streams as `f32`.
pub fn fixed_to_f32(n: i64, decimals: u8) -> f32 {
    if n.unsigned_abs() < (1 << 24) && decimals <= 10 {
        n as f32 / POW10_F32[decimals as usize]
    } else {
        (n as f64 / 10f64.powi(decimals as i32)) as f32
    }
}

#[cfg(test)]
mod tests;
