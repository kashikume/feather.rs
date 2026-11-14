//! Reading `.fmesh` files.

use super::*;

/// A mesh read from an `.fmesh` file.
#[derive(Clone, Debug, PartialEq)]
pub struct Fmesh {
    pub header: Header,
    /// x, y, z per vertex.
    pub positions: Vec<f32>,
    /// x, y, z per vertex.
    pub normals: Vec<f32>,
    /// u, v per vertex, V already flipped (0 at the top of the image).
    pub uvs: Vec<f32>,
    /// Three per triangle.
    pub indices: Vec<u32>,
}

impl Fmesh {
    pub fn vertex_count(&self) -> usize {
        self.header.vertex_count as usize
    }
}

/// Reads and checks the header of an `.fmesh` file, without the data.
pub fn read_header(bytes: &[u8]) -> Result<Header, FmeshError> {
    if bytes.len() < 12 {
        return Err(if bytes.len() >= 8 && bytes[..8] != MAGIC {
            FmeshError::BadMagic
        } else {
            FmeshError::Truncated("header")
        });
    }
    if bytes[..8] != MAGIC {
        return Err(FmeshError::BadMagic);
    }
    let mut reader = Reader { bytes, pos: 8 };
    let version = reader.u16();
    if version != VERSION {
        return Err(FmeshError::UnsupportedVersion {
            found: version,
            supported: VERSION,
        });
    }
    let header_size = reader.u16();
    if (header_size as usize) < HEADER_SIZE {
        return Err(FmeshError::BadHeaderSize(header_size));
    }
    if bytes.len() < header_size as usize {
        return Err(FmeshError::Truncated("header"));
    }

    let flags = reader.u32();
    if flags & !KNOWN_FLAGS != 0 {
        return Err(FmeshError::UnknownFlags(flags & !KNOWN_FLAGS));
    }
    let vertex_count = reader.u32();
    let index_count = reader.u32();
    let positions = reader.descriptor("positions", false)?;
    let normals = reader.descriptor("normals", false)?;
    let uvs = reader.descriptor("uvs", false)?;
    let indices = reader.descriptor("indices", true)?;
    let mut bounds = [0f32; 6];
    for value in &mut bounds {
        *value = reader.f32();
    }
    let header = Header {
        version,
        header_size,
        flags,
        vertex_count,
        index_count,
        positions,
        normals,
        uvs,
        indices,
        bounds_min: [bounds[0], bounds[1], bounds[2]],
        bounds_max: [bounds[3], bounds[4], bounds[5]],
        data_size: reader.u32(),
        stored_size: reader.u32(),
        crc32: reader.u32(),
    };

    if !index_count.is_multiple_of(3) {
        return Err(FmeshError::Invalid(format!(
            "{index_count} indices, not a multiple of 3"
        )));
    }
    if header.data_size > MAX_DATA_SIZE {
        return Err(FmeshError::Invalid(format!(
            "data size {} is larger than {MAX_DATA_SIZE} bytes",
            header.data_size
        )));
    }
    // Every value takes at least one byte, which bounds the counts before anything is
    // allocated for them.
    let values = 8 * vertex_count as u64 + index_count as u64;
    if values > header.data_size as u64 {
        return Err(FmeshError::Invalid(format!(
            "{vertex_count} vertices and {index_count} indices can't fit in {} bytes of data",
            header.data_size
        )));
    }
    if !header.compressed() && header.stored_size != header.data_size {
        return Err(FmeshError::Invalid(format!(
            "uncompressed data with stored size {} and data size {}",
            header.stored_size, header.data_size
        )));
    }
    let expected_len = header_size as u64 + header.stored_size as u64;
    if (bytes.len() as u64) < expected_len {
        return Err(FmeshError::Truncated("data"));
    }
    if bytes.len() as u64 > expected_len {
        return Err(FmeshError::Invalid(format!(
            "{} bytes after the data",
            bytes.len() as u64 - expected_len
        )));
    }
    Ok(header)
}

/// Reads an `.fmesh` file: checks it entirely (header, checksum, sizes, every index within
/// the vertices) and decodes its streams.
pub fn read(bytes: &[u8]) -> Result<Fmesh, FmeshError> {
    let header = read_header(bytes)?;
    let stored = &bytes[header.header_size as usize..];

    let crc = crc32fast::hash(stored);
    if crc != header.crc32 {
        return Err(FmeshError::Checksum {
            expected: header.crc32,
            found: crc,
        });
    }

    let inflated;
    let data = if header.compressed() {
        inflated =
            miniz_oxide::inflate::decompress_to_vec_with_limit(stored, header.data_size as usize)
                .map_err(|_| FmeshError::Decompress)?;
        if inflated.len() != header.data_size as usize {
            return Err(FmeshError::Decompress);
        }
        &inflated[..]
    } else {
        stored
    };

    let vertex_count = header.vertex_count as usize;
    let mut reader = Reader {
        bytes: data,
        pos: 0,
    };
    let positions = reader.vertex_stream(
        "positions",
        header.positions,
        vertex_count,
        POSITION_COMPONENTS,
    )?;
    let normals =
        reader.vertex_stream("normals", header.normals, vertex_count, NORMAL_COMPONENTS)?;
    let uvs = reader.vertex_stream("uvs", header.uvs, vertex_count, UV_COMPONENTS)?;
    let indices = reader.indices(header.indices, header.index_count as usize)?;
    if reader.pos != data.len() {
        return Err(FmeshError::Invalid(format!(
            "{} bytes of data left after the streams",
            data.len() - reader.pos
        )));
    }
    if let Some(&index) = indices.iter().find(|&&i| i as usize >= vertex_count) {
        return Err(FmeshError::Invalid(format!(
            "index {index} points past the {vertex_count} vertices"
        )));
    }

    Ok(Fmesh {
        header,
        positions,
        normals,
        uvs,
        indices,
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    /// The next `N` bytes. Only used where the length was checked before (the header).
    fn array<const N: usize>(&mut self) -> [u8; N] {
        let array = self.bytes[self.pos..self.pos + N].try_into().unwrap();
        self.pos += N;
        array
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.array())
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.array())
    }

    fn f32(&mut self) -> f32 {
        f32::from_le_bytes(self.array())
    }

    fn descriptor(
        &mut self,
        stream: &'static str,
        indices: bool,
    ) -> Result<StreamDescriptor, FmeshError> {
        let [code, decimals, _, _] = self.array::<4>();
        let encoding = Encoding::from_code(code)
            .filter(|e| {
                if indices {
                    e.for_indices()
                } else {
                    e.for_vertices()
                }
            })
            .ok_or(FmeshError::BadEncoding { stream, code })?;
        let max_decimals = if indices || encoding == Encoding::F32 {
            0
        } else {
            MAX_DECIMALS
        };
        if decimals > max_decimals {
            return Err(FmeshError::BadDecimals { stream, decimals });
        }
        Ok(StreamDescriptor { encoding, decimals })
    }

    /// The next `len` bytes of the data, or an error naming `stream` if there aren't enough.
    fn take(&mut self, len: usize, stream: &'static str) -> Result<&[u8], FmeshError> {
        if self.bytes.len() - self.pos < len {
            return Err(FmeshError::Invalid(format!(
                "{stream}: the data ends inside the stream"
            )));
        }
        let slice = &self.bytes[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn varint(&mut self, stream: &'static str) -> Result<u64, FmeshError> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = *self.bytes.get(self.pos).ok_or_else(|| {
                FmeshError::Invalid(format!("{stream}: the data ends inside the stream"))
            })?;
            self.pos += 1;
            value |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(FmeshError::Invalid(format!(
            "{stream}: varint longer than 10 bytes"
        )))
    }

    /// Decodes `count` delta values of `components` components each, as `i64`s.
    fn deltas(
        &mut self,
        stream: &'static str,
        count: usize,
        components: usize,
    ) -> Result<Vec<i64>, FmeshError> {
        let mut previous = vec![0i64; components];
        let mut values = Vec::with_capacity(count);
        for i in 0..count {
            let encoded = self.varint(stream)?;
            let delta = ((encoded >> 1) as i64) ^ -((encoded & 1) as i64);
            let slot = &mut previous[i % components];
            *slot = slot
                .checked_add(delta)
                .ok_or_else(|| FmeshError::Invalid(format!("{stream}: delta overflows")))?;
            values.push(*slot);
        }
        Ok(values)
    }

    fn vertex_stream(
        &mut self,
        stream: &'static str,
        descriptor: StreamDescriptor,
        vertex_count: usize,
        components: usize,
    ) -> Result<Vec<f32>, FmeshError> {
        let count = vertex_count * components;
        let d = descriptor.decimals;
        let fixed = |n: i64| fixed_to_f32(n, d);
        Ok(match descriptor.encoding {
            Encoding::F32 => self
                .take(count * 4, stream)?
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect(),
            Encoding::I8 => self
                .take(count, stream)?
                .iter()
                .map(|&b| fixed(b as i8 as i64))
                .collect(),
            Encoding::I16 => self
                .take(count * 2, stream)?
                .chunks_exact(2)
                .map(|b| fixed(i16::from_le_bytes(b.try_into().unwrap()) as i64))
                .collect(),
            Encoding::I32 => self
                .take(count * 4, stream)?
                .chunks_exact(4)
                .map(|b| fixed(i32::from_le_bytes(b.try_into().unwrap()) as i64))
                .collect(),
            Encoding::Delta => {
                let values = self.deltas(stream, count, components)?;
                if values
                    .iter()
                    .any(|&v| v < i32::MIN as i64 || v > i32::MAX as i64)
                {
                    return Err(FmeshError::Invalid(format!("{stream}: value outside i32")));
                }
                values.into_iter().map(fixed).collect()
            }
            Encoding::U16 | Encoding::U32 => unreachable!("checked by descriptor()"),
        })
    }

    fn indices(
        &mut self,
        descriptor: StreamDescriptor,
        count: usize,
    ) -> Result<Vec<u32>, FmeshError> {
        const STREAM: &str = "indices";
        Ok(match descriptor.encoding {
            Encoding::U16 => self
                .take(count * 2, STREAM)?
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()) as u32)
                .collect(),
            Encoding::U32 => self
                .take(count * 4, STREAM)?
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect(),
            Encoding::Delta => self
                .deltas(STREAM, count, 1)?
                .into_iter()
                .map(|v| {
                    u32::try_from(v)
                        .map_err(|_| FmeshError::Invalid("indices: value outside u32".into()))
                })
                .collect::<Result<_, _>>()?,
            _ => unreachable!("checked by descriptor()"),
        })
    }
}
