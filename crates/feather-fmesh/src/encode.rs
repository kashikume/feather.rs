//! Writing `.fmesh` files.

use super::*;

/// A written file and its header (which tells the encodings chosen and the sizes).
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    pub bytes: Vec<u8>,
    pub header: Header,
}

/// Makes the bytes of an `.fmesh` file holding `mesh`.
///
/// Fails if the streams' lengths don't match the vertex count, the index count isn't a
/// multiple of 3, an index points past the vertices, a stream has more than
/// [`MAX_DECIMALS`] decimals, or the data is larger than [`MAX_DATA_SIZE`].
pub fn write(mesh: &MeshData, options: &WriteOptions) -> Result<Written, FmeshError> {
    let vertex_count = mesh.vertex_count();
    check_lengths(mesh, vertex_count)?;

    let mut data = Vec::new();
    let positions = encode_stream(
        &mesh.positions,
        POSITION_COMPONENTS,
        options.encoding,
        &mut data,
    );
    let normals = encode_stream(
        &mesh.normals,
        NORMAL_COMPONENTS,
        options.encoding,
        &mut data,
    );
    let uvs = encode_stream(&mesh.uvs, UV_COMPONENTS, options.encoding, &mut data);
    let indices = encode_indices(&mesh.indices, vertex_count, options.encoding, &mut data);

    if data.len() > MAX_DATA_SIZE as usize {
        return Err(FmeshError::Invalid(format!(
            "the data ({} bytes) is larger than {MAX_DATA_SIZE} bytes",
            data.len()
        )));
    }
    let data_size = data.len() as u32;

    let compressed = match options.compression {
        Compression::Off => None,
        Compression::On => Some(miniz_oxide::deflate::compress_to_vec(&data, options.level)),
        Compression::Auto => {
            let deflated = miniz_oxide::deflate::compress_to_vec(&data, options.level);
            let saving = 1.0 - deflated.len() as f64 / data.len().max(1) as f64;
            (saving >= AUTO_COMPRESS_MIN_SAVING).then_some(deflated)
        }
    };
    let (flags, stored) = match compressed {
        Some(deflated) => (FLAG_COMPRESSED, deflated),
        None => (0, data),
    };

    let (bounds_min, bounds_max) = bounds(&mesh.positions.to_f32());
    let header = Header {
        version: VERSION,
        header_size: HEADER_SIZE as u16,
        flags,
        vertex_count: vertex_count as u32,
        index_count: mesh.indices.len() as u32,
        positions,
        normals,
        uvs,
        indices,
        bounds_min,
        bounds_max,
        data_size,
        stored_size: stored.len() as u32,
        crc32: crc32fast::hash(&stored),
    };

    let mut bytes = Vec::with_capacity(HEADER_SIZE + stored.len());
    write_header(&header, &mut bytes);
    bytes.extend_from_slice(&stored);
    Ok(Written { bytes, header })
}

fn check_lengths(mesh: &MeshData, vertex_count: usize) -> Result<(), FmeshError> {
    let invalid = |message: String| Err(FmeshError::Invalid(message));
    if vertex_count > u32::MAX as usize || mesh.indices.len() > u32::MAX as usize {
        return invalid("too many vertices or indices".into());
    }
    let streams = [
        ("positions", &mesh.positions, POSITION_COMPONENTS),
        ("normals", &mesh.normals, NORMAL_COMPONENTS),
        ("uvs", &mesh.uvs, UV_COMPONENTS),
    ];
    for (name, stream, components) in streams {
        if stream.len() != vertex_count * components {
            return invalid(format!(
                "{name}: {} values, expected {} ({vertex_count} vertices of {components})",
                stream.len(),
                vertex_count * components
            ));
        }
        if let Stream::Fixed { decimals, .. } = stream {
            if *decimals > MAX_DECIMALS {
                return Err(FmeshError::BadDecimals {
                    stream: name,
                    decimals: *decimals,
                });
            }
        }
    }
    if !mesh.indices.len().is_multiple_of(3) {
        return invalid(format!(
            "{} indices, not a multiple of 3",
            mesh.indices.len()
        ));
    }
    if let Some(&index) = mesh.indices.iter().find(|&&i| i as usize >= vertex_count) {
        return invalid(format!(
            "index {index} points past the {vertex_count} vertices"
        ));
    }
    Ok(())
}

/// Appends `stream` to `data` in the encoding `choice` asks for, and returns its descriptor.
fn encode_stream(
    stream: &Stream,
    components: usize,
    choice: EncodingChoice,
    data: &mut Vec<u8>,
) -> StreamDescriptor {
    let (values, decimals) = match (stream, choice) {
        (Stream::Float(_), _) | (Stream::Fixed { .. }, EncodingChoice::F32) => {
            for value in stream.to_f32() {
                data.extend_from_slice(&value.to_le_bytes());
            }
            return StreamDescriptor {
                encoding: Encoding::F32,
                decimals: 0,
            };
        }
        (Stream::Fixed { values, decimals }, _) => (values, *decimals),
    };

    let fixed = narrowest(values);
    let encoding = match choice {
        EncodingChoice::Int => fixed,
        EncodingChoice::Delta => Encoding::Delta,
        _ => {
            let fixed_size = values.len() * fixed.width().unwrap();
            let mut delta = Vec::new();
            write_deltas(values.iter().map(|&v| v as i64), components, &mut delta);
            if delta.len() < fixed_size {
                data.extend_from_slice(&delta);
                return StreamDescriptor {
                    encoding: Encoding::Delta,
                    decimals,
                };
            }
            fixed
        }
    };

    match encoding {
        Encoding::I8 => data.extend(values.iter().map(|&v| v as i8 as u8)),
        Encoding::I16 => values
            .iter()
            .for_each(|&v| data.extend_from_slice(&(v as i16).to_le_bytes())),
        Encoding::I32 => values
            .iter()
            .for_each(|&v| data.extend_from_slice(&v.to_le_bytes())),
        Encoding::Delta => write_deltas(values.iter().map(|&v| v as i64), components, data),
        _ => unreachable!(),
    }
    StreamDescriptor { encoding, decimals }
}

/// The narrowest of `I8`, `I16` and `I32` holding every value.
fn narrowest(values: &[i32]) -> Encoding {
    let fits = |min: i32, max: i32| values.iter().all(|&v| (min..=max).contains(&v));
    if fits(i8::MIN as i32, i8::MAX as i32) {
        Encoding::I8
    } else if fits(i16::MIN as i32, i16::MAX as i32) {
        Encoding::I16
    } else {
        Encoding::I32
    }
}

fn encode_indices(
    indices: &[u32],
    vertex_count: usize,
    choice: EncodingChoice,
    data: &mut Vec<u8>,
) -> StreamDescriptor {
    let fixed = if vertex_count <= 1 << 16 {
        Encoding::U16
    } else {
        Encoding::U32
    };
    let encoding = match choice {
        EncodingChoice::Delta => Encoding::Delta,
        EncodingChoice::Auto => {
            let mut delta = Vec::new();
            write_deltas(indices.iter().map(|&i| i as i64), 1, &mut delta);
            if delta.len() < indices.len() * fixed.width().unwrap() {
                data.extend_from_slice(&delta);
                return StreamDescriptor {
                    encoding: Encoding::Delta,
                    decimals: 0,
                };
            }
            fixed
        }
        EncodingChoice::F32 | EncodingChoice::Int => fixed,
    };
    match encoding {
        Encoding::U16 => indices
            .iter()
            .for_each(|&i| data.extend_from_slice(&(i as u16).to_le_bytes())),
        Encoding::U32 => indices
            .iter()
            .for_each(|&i| data.extend_from_slice(&i.to_le_bytes())),
        Encoding::Delta => write_deltas(indices.iter().map(|&i| i as i64), 1, data),
        _ => unreachable!(),
    }
    StreamDescriptor {
        encoding,
        decimals: 0,
    }
}

/// Writes each value as the zigzag varint of its difference from the value `components`
/// places earlier (from 0 for the first vertex).
fn write_deltas(values: impl Iterator<Item = i64>, components: usize, data: &mut Vec<u8>) {
    let mut previous = vec![0i64; components];
    for (i, value) in values.enumerate() {
        let slot = &mut previous[i % components];
        write_varint(zigzag(value - *slot), data);
        *slot = value;
    }
}

/// Maps signed integers to unsigned ones, small magnitudes to small numbers:
/// 0, -1, 1, -2, 2... to 0, 1, 2, 3, 4...
pub(crate) fn zigzag(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

/// LEB128: 7 bits per byte, least significant first, the top bit set on every byte but the
/// last.
pub(crate) fn write_varint(mut value: u64, data: &mut Vec<u8>) {
    while value >= 0x80 {
        data.push((value as u8) | 0x80);
        value >>= 7;
    }
    data.push(value as u8);
}

fn bounds(positions: &[f32]) -> ([f32; 3], [f32; 3]) {
    if positions.is_empty() {
        return ([0.0; 3], [0.0; 3]);
    }
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for point in positions.chunks_exact(3) {
        for c in 0..3 {
            min[c] = min[c].min(point[c]);
            max[c] = max[c].max(point[c]);
        }
    }
    (min, max)
}

fn write_header(header: &Header, bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&header.version.to_le_bytes());
    bytes.extend_from_slice(&header.header_size.to_le_bytes());
    bytes.extend_from_slice(&header.flags.to_le_bytes());
    bytes.extend_from_slice(&header.vertex_count.to_le_bytes());
    bytes.extend_from_slice(&header.index_count.to_le_bytes());
    for stream in [header.positions, header.normals, header.uvs, header.indices] {
        bytes.extend_from_slice(&[stream.encoding.code(), stream.decimals, 0, 0]);
    }
    for value in header.bounds_min.iter().chain(&header.bounds_max) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&header.data_size.to_le_bytes());
    bytes.extend_from_slice(&header.stored_size.to_le_bytes());
    bytes.extend_from_slice(&header.crc32.to_le_bytes());
    debug_assert_eq!(bytes.len(), HEADER_SIZE);
}
