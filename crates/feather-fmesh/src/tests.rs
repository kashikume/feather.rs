use super::encode::{write_varint, zigzag};
use super::*;

/// One triangle, as in `doc/fmesh-format.md`'s example.
fn triangle() -> MeshData {
    MeshData {
        positions: Stream::Fixed { values: vec![0, 0, 0, 10, 0, 0, 0, 10, 0], decimals: 1 },
        normals: Stream::Fixed { values: vec![0, 0, 1, 0, 0, 1, 0, 0, 1], decimals: 0 },
        uvs: Stream::Fixed { values: vec![0, 1, 1, 1, 0, 0], decimals: 0 },
        indices: vec![0, 1, 2],
    }
}

/// A mesh with every kind of value: large, negative, many decimals, many vertices.
fn big_mesh(vertex_count: usize) -> MeshData {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    for i in 0..vertex_count as i32 {
        positions.extend([i * 1234 - 500_000, (i * 37) % 9001 - 4500, -i]);
        normals.extend([(i % 3) - 1, 1000 - i % 2001, 7]);
        uvs.extend([i % 1_000_000, 1_000_000 - i % 1_000_000]);
    }
    let indices = (0..vertex_count as u32 / 3 * 3).rev().collect();
    MeshData {
        positions: Stream::Fixed { values: positions, decimals: 3 },
        normals: Stream::Fixed { values: normals, decimals: 3 },
        uvs: Stream::Fixed { values: uvs, decimals: 6 },
        indices,
    }
}

fn options(encoding: EncodingChoice, compression: Compression) -> WriteOptions {
    WriteOptions { encoding, compression, ..WriteOptions::default() }
}

fn round_trip(mesh: &MeshData, options: &WriteOptions) -> Fmesh {
    let written = write(mesh, options).unwrap();
    let read = read(&written.bytes).unwrap();
    assert_eq!(read.header, written.header);
    assert_eq!(read.positions, mesh.positions.to_f32());
    assert_eq!(read.normals, mesh.normals.to_f32());
    assert_eq!(read.uvs, mesh.uvs.to_f32());
    assert_eq!(read.indices, mesh.indices);
    read
}

#[test]
fn round_trips_with_every_encoding_and_compression() {
    for mesh in [triangle(), big_mesh(70_000)] {
        for encoding in [EncodingChoice::Auto, EncodingChoice::F32, EncodingChoice::Int, EncodingChoice::Delta] {
            for compression in [Compression::Auto, Compression::On, Compression::Off] {
                round_trip(&mesh, &options(encoding, compression));
            }
        }
    }
}

#[test]
fn picks_encodings() {
    let read = round_trip(&triangle(), &options(EncodingChoice::Int, Compression::Off));
    assert_eq!(read.header.positions, StreamDescriptor { encoding: Encoding::I8, decimals: 1 });
    assert_eq!(read.header.indices.encoding, Encoding::U16);

    let read = round_trip(&big_mesh(70_000), &options(EncodingChoice::Int, Compression::Off));
    assert_eq!(read.header.positions.encoding, Encoding::I32);
    assert_eq!(read.header.normals.encoding, Encoding::I16);
    assert_eq!(read.header.indices.encoding, Encoding::U32);

    let read = round_trip(&big_mesh(100), &options(EncodingChoice::F32, Compression::Off));
    assert_eq!(read.header.positions, StreamDescriptor { encoding: Encoding::F32, decimals: 0 });

    let read = round_trip(&big_mesh(100), &options(EncodingChoice::Delta, Compression::Off));
    assert_eq!(read.header.uvs.encoding, Encoding::Delta);
    assert_eq!(read.header.indices.encoding, Encoding::Delta);
}

#[test]
fn auto_takes_the_smaller_encoding() {
    let mesh = big_mesh(3000);
    let size = |encoding| write(&mesh, &options(encoding, Compression::Off)).unwrap().header.data_size;
    let auto = size(EncodingChoice::Auto);
    assert!(auto <= size(EncodingChoice::Int));
    assert!(auto <= size(EncodingChoice::Delta));
}

#[test]
fn float_streams_stay_float() {
    let mut mesh = triangle();
    mesh.positions = Stream::Float(vec![0.1, 0.2, 0.3, 1e30, -0.5, 0.0, f32::MIN_POSITIVE, 2.0, 3.0]);
    let read = round_trip(&mesh, &options(EncodingChoice::Int, Compression::Off));
    assert_eq!(read.header.positions.encoding, Encoding::F32);
    assert_eq!(read.header.bounds_max, [1e30, 2.0, 3.0]);
}

#[test]
fn bounds_cover_the_positions() {
    let header = write(&triangle(), &WriteOptions::default()).unwrap().header;
    assert_eq!(header.bounds_min, [0.0, 0.0, 0.0]);
    assert_eq!(header.bounds_max, [1.0, 1.0, 0.0]);
}

#[test]
fn empty_mesh() {
    let mesh = MeshData {
        positions: Stream::Fixed { values: vec![], decimals: 0 },
        normals: Stream::Float(vec![]),
        uvs: Stream::Float(vec![]),
        indices: vec![],
    };
    let read = round_trip(&mesh, &WriteOptions::default());
    assert_eq!(read.vertex_count(), 0);
}

#[test]
fn compression_auto_keeps_only_worthwhile_compression() {
    // Repetitive data compresses well.
    let header = write(&big_mesh(30_000), &WriteOptions::default()).unwrap().header;
    assert!(header.compressed());
    // Random floats don't.
    let mut seed = 12345u32;
    let mut random = |count: usize| {
        (0..count)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                f32::from_bits(seed)
            })
            .collect::<Vec<_>>()
    };
    let mesh = MeshData {
        positions: Stream::Float(random(3000)),
        normals: Stream::Float(random(3000)),
        uvs: Stream::Float(random(2000)),
        indices: vec![0, 1, 2],
    };
    let header = write(&mesh, &WriteOptions::default()).unwrap().header;
    assert!(!header.compressed());
}

#[test]
fn fixed_to_f32_matches_parsing_the_decimal() {
    let cases = [
        (573651, 6, "0.573651"),
        (-573651, 6, "-0.573651"),
        (29, 2, "0.29"),
        (1, 1, "0.1"),
        (-111966, 3, "-111.966"),
        (16777215, 7, "1.6777215"),
        (123, 0, "123"),
        (7, 10, "0.0000000007"),
    ];
    for (n, d, text) in cases {
        assert_eq!(fixed_to_f32(n, d), text.parse::<f32>().unwrap(), "{text}");
    }
    // Exhaustively for small ones.
    for d in 0..=6u8 {
        for n in -20_000..20_000i64 {
            let text = format!("{}e-{d}", n);
            assert_eq!(fixed_to_f32(n, d), text.parse::<f32>().unwrap(), "{text}");
        }
    }
}

#[test]
fn varints() {
    let encode = |v: i64| {
        let mut data = Vec::new();
        write_varint(zigzag(v), &mut data);
        data
    };
    assert_eq!(encode(0), [0]);
    assert_eq!(encode(-1), [1]);
    assert_eq!(encode(1), [2]);
    assert_eq!(encode(63), [126]);
    assert_eq!(encode(64), [0x80, 1]);
    assert_eq!(encode(i32::MIN as i64 - i32::MAX as i64).len(), 5);
}

fn written_triangle() -> Vec<u8> {
    write(&triangle(), &options(EncodingChoice::Int, Compression::Off)).unwrap().bytes
}

#[test]
fn rejects_wrong_magic() {
    let mut bytes = written_triangle();
    bytes[5] = b'\n'; // "\r\n" turned into "\n\n", as a text-mode transfer would
    assert_eq!(read(&bytes), Err(FmeshError::BadMagic));
    assert_eq!(read(b"v 0 0 0\nv 1 0 0\n"), Err(FmeshError::BadMagic));
}

#[test]
fn rejects_unknown_version() {
    let mut bytes = written_triangle();
    bytes[8] = 2;
    assert_eq!(read(&bytes), Err(FmeshError::UnsupportedVersion { found: 2, supported: 1 }));
}

#[test]
fn rejects_unknown_flags() {
    let mut bytes = written_triangle();
    bytes[12] |= 0x4;
    assert_eq!(read(&bytes), Err(FmeshError::UnknownFlags(0x4)));
}

#[test]
fn rejects_bad_encodings() {
    let mut bytes = written_triangle();
    bytes[24] = Encoding::U16.code(); // positions as indices
    assert!(matches!(read(&bytes), Err(FmeshError::BadEncoding { stream: "positions", .. })));
    let mut bytes = written_triangle();
    bytes[36] = Encoding::I16.code(); // indices as a vertex stream
    assert!(matches!(read(&bytes), Err(FmeshError::BadEncoding { stream: "indices", .. })));
    let mut bytes = written_triangle();
    bytes[25] = MAX_DECIMALS + 1;
    assert!(matches!(read(&bytes), Err(FmeshError::BadDecimals { .. })));
}

#[test]
fn rejects_truncated_files() {
    let bytes = written_triangle();
    for len in [0, 4, 11, 40, HEADER_SIZE, bytes.len() - 1] {
        assert!(matches!(read(&bytes[..len]), Err(FmeshError::Truncated(_))), "{len} bytes");
    }
}

#[test]
fn rejects_extra_bytes() {
    let mut bytes = written_triangle();
    bytes.push(0);
    assert!(matches!(read(&bytes), Err(FmeshError::Invalid(_))));
}

#[test]
fn rejects_bad_checksum() {
    let mut bytes = written_triangle();
    *bytes.last_mut().unwrap() ^= 1;
    assert!(matches!(read(&bytes), Err(FmeshError::Checksum { .. })));

    let mut bytes = write(&big_mesh(30_000), &options(EncodingChoice::Auto, Compression::On)).unwrap().bytes;
    bytes[HEADER_SIZE + 100] ^= 0x10;
    assert!(matches!(read(&bytes), Err(FmeshError::Checksum { .. })));
}

#[test]
fn rejects_impossible_counts() {
    let mut bytes = written_triangle();
    bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(read(&bytes), Err(FmeshError::Invalid(_))));
    let mut bytes = written_triangle();
    bytes[20] = 4; // 4 indices
    assert!(matches!(read(&bytes), Err(FmeshError::Invalid(_))));
}

#[test]
fn rejects_indices_past_the_vertices() {
    let mut mesh = triangle();
    mesh.indices = vec![0, 1, 3];
    assert!(matches!(write(&mesh, &WriteOptions::default()), Err(FmeshError::Invalid(_))));

    // The same in a file: patch the last index (u16) to 3 and fix the checksum.
    let mut bytes = written_triangle();
    let len = bytes.len();
    bytes[len - 2] = 3;
    let crc = crc32fast::hash(&bytes[HEADER_SIZE..]);
    bytes[72..76].copy_from_slice(&crc.to_le_bytes());
    assert!(matches!(read(&bytes), Err(FmeshError::Invalid(_))));
}

#[test]
fn rejects_mismatched_stream_lengths() {
    let mut mesh = triangle();
    mesh.uvs = Stream::Float(vec![0.0; 4]);
    assert!(matches!(write(&mesh, &WriteOptions::default()), Err(FmeshError::Invalid(_))));
}

#[test]
fn reads_a_larger_header() {
    // A later version could add fields to the header; a reader of version 1 skips them.
    // (Simulated with version 1, as only the size matters here.)
    let bytes = written_triangle();
    let mut bigger = bytes[..HEADER_SIZE].to_vec();
    bigger[10..12].copy_from_slice(&((HEADER_SIZE + 8) as u16).to_le_bytes());
    bigger.extend_from_slice(&[0xee; 8]);
    bigger.extend_from_slice(&bytes[HEADER_SIZE..]);
    let mesh = read(&bigger).unwrap();
    assert_eq!(mesh.indices, vec![0, 1, 2]);
}

/// The example of `doc/fmesh-format.md`: the bytes of its table are `testdata/triangle.fmesh`
/// (which `mesh-converter`'s tests make again from `testdata/triangle.obj`).
#[test]
fn example_in_the_documentation() {
    let doc = include_str!("../../../doc/fmesh-format.md");
    let start = doc.find("<!-- example:start -->").unwrap();
    let end = doc.find("<!-- example:end -->").unwrap();
    let mut bytes = Vec::new();
    let mut offset = 0;
    for row in doc[start..end].lines().filter(|l| l.starts_with("| ") && !l.starts_with("| Offset")) {
        let columns: Vec<&str> = row.split('|').map(str::trim).collect();
        assert_eq!(columns[1].parse::<usize>().unwrap(), offset, "offset of row {row}");
        for byte in columns[2].trim_matches('`').split(' ') {
            bytes.push(u8::from_str_radix(byte, 16).unwrap());
        }
        offset = bytes.len();
    }
    assert_eq!(bytes, include_bytes!("../testdata/triangle.fmesh"));

    let mesh = read(&bytes).unwrap();
    assert_eq!(mesh.positions, [0.0, 0.0, 0.0, 1.5, 0.0, 0.0, 0.0, 1.25, 0.0]);
    assert_eq!(mesh.normals, [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
    assert_eq!(mesh.uvs, [0.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
    assert_eq!(mesh.indices, [0, 1, 2]);
    assert_eq!(mesh.header.bounds_max, [1.5, 1.25, 0.0]);
}

/// The CRC-32 is the one `doc/fmesh-format.md` names.
#[test]
fn crc32_is_the_zip_one() {
    assert_eq!(crc32fast::hash(b"123456789"), 0xCBF4_3926);
}
