use super::*;
use feather_fmesh::{Encoding, Stream};

const TRIANGLE: &str = "v 0 0 0
v 1 0 0
v 0 1 0
vt 0 0
vt 1 0
vt 0 1
vn 0 0 1
f 1/1/1 2/2/1 3/3/1
";

/// A unit cube, each face a quad, without normals.
const CUBE: &str = "o Cube
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
v 0 0 1
v 1 0 1
v 1 1 1
v 0 1 1
f 1 4 3 2
f 5 6 7 8
f 1 2 6 5
f 2 3 7 6
f 3 4 8 7
f 4 1 5 8
";

fn one(text: &str, settings: &Settings) -> Converted {
    let mut conversion = convert(text, settings).unwrap();
    assert_eq!(conversion.meshes.len(), 1);
    let converted = conversion.meshes.remove(0);
    check(&converted, &converted.written.bytes).unwrap();
    converted
}

fn read(converted: &Converted) -> feather_fmesh::Fmesh {
    feather_fmesh::read(&converted.written.bytes).unwrap()
}

#[test]
fn converts_a_triangle() {
    let converted = one(TRIANGLE, &Settings::default());
    let mesh = read(&converted);
    assert_eq!(mesh.positions, [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    assert_eq!(mesh.normals, [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
    // V flipped.
    assert_eq!(mesh.uvs, [0.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
    assert_eq!(mesh.indices, [0, 1, 2]);
    assert_eq!(converted.stats, Stats { triangles: 1, corners: 3, vertices: 3 });
}

#[test]
fn keeps_v_with_no_flip() {
    let converted = one(TRIANGLE, &Settings { flip_v: false, ..Settings::default() });
    assert_eq!(read(&converted).uvs, [0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn flips_v_exactly() {
    let text = "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0.198708 0.637534\nvn 0 0 1\nf 1/1/1 2/1/1 3/1/1\n";
    let converted = one(text, &Settings::default());
    assert_eq!(read(&converted).uvs[1], "0.362466".parse::<f32>().unwrap());
}

#[test]
fn precision_per_stream() {
    let text = "v 0.573651 0.001530 -12.5
v 1 0 0
v 0 1 0
vt 0.5 0.25
vn 1.0000 0.0000 -0.0000
f 1/1/1 2/1/1 3/1/1
";
    let converted = one(text, &Settings { encoding: EncodingChoice::Int, ..Settings::default() });
    let header = read(&converted).header;
    assert_eq!((header.positions.encoding, header.positions.decimals), (Encoding::I32, 6));
    assert_eq!((header.normals.encoding, header.normals.decimals), (Encoding::I8, 0));
    assert_eq!((header.uvs.encoding, header.uvs.decimals), (Encoding::I8, 2));
    assert!(converted.warnings.is_empty());
}

#[test]
fn values_read_back_as_parsed() {
    // Values a float can't hold exactly read back as the nearest float, like parsing them.
    let text = "v 0.1 0.2 0.3
v -111.966 120.178 3.986
v 0.29 1.5e-3 7
vt 0 0
vn 0 0 1
f 1/1/1 2/1/1 3/1/1
";
    let converted = one(text, &Settings::default());
    let mesh = read(&converted);
    let expected: Vec<f32> = "0.1 0.2 0.3 -111.966 120.178 3.986 0.29 1.5e-3 7"
        .split(' ')
        .map(|t| t.parse().unwrap())
        .collect();
    assert_eq!(mesh.positions, expected);
}

#[test]
fn too_many_decimals_fall_back_to_floats() {
    let text = "v 0.1000000015 0 0\nv 0.1 0 0\nv 0 0.1 0\nvt 0 0\nvn 0 0 1\nf 1/1/1 2/1/1 3/1/1\n";
    let converted = one(text, &Settings::default());
    assert_eq!(read(&converted).header.positions.encoding, Encoding::F32);
    assert!(converted.warnings[0].contains("--max-decimals"), "{:?}", converted.warnings);
    assert_eq!(read(&converted).positions[0], "0.1000000015".parse::<f32>().unwrap());

    let converted = one(text, &Settings { max_decimals: 10, ..Settings::default() });
    assert_eq!(read(&converted).header.positions.decimals, 10);
}

#[test]
fn merges_identical_vertices() {
    // Two triangles of a quad: 4 vertices. The second "0.0" and "-0" are the same as "0".
    let text = "v 0 0 0
v 1 0 0
v 1 1 0
v 0.0 -0 0.000
vt 0 0
vn 0 0 1
f 1/1/1 2/1/1 3/1/1
f 4/1/1 3/1/1 -3/1/1
";
    let converted = one(text, &Settings::default());
    assert_eq!(converted.stats.vertices, 3);
    assert_eq!(read(&converted).indices, [0, 1, 2, 0, 2, 1]);
}

#[test]
fn triangulates_polygons_as_fans() {
    let text = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nv -1 0.5 0\nvt 0 0\nvn 0 0 1\nf 1/1/1 2/1/1 3/1/1 4/1/1 5/1/1\n";
    let converted = one(text, &Settings::default());
    assert_eq!(read(&converted).indices, [0, 1, 2, 0, 2, 3, 0, 3, 4]);
    assert_eq!(converted.notes, ["line 8: polygon of 5 corners split into 3 triangles"]);
}

#[test]
fn missing_uvs_are_zero() {
    let text = "v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nf 1//1 2//1 3//1\n";
    let converted = one(text, &Settings::default());
    assert_eq!(read(&converted).uvs, [0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
}

#[test]
fn missing_normals_are_an_error() {
    let error = convert(CUBE, &Settings::default()).err().unwrap().to_string();
    assert!(error.contains("--flat-normals"), "{error}");
}

#[test]
fn flat_normals_of_a_cube() {
    let converted = one(CUBE, &Settings { normals: Normals::Flat, ..Settings::default() });
    let mesh = read(&converted);
    assert_eq!(converted.stats.vertices, 24);
    assert_eq!(converted.stats.triangles, 12);
    // Every normal along an axis, pointing out of the cube.
    for (i, normal) in mesh.normals.chunks(3).enumerate() {
        assert_eq!(normal.iter().map(|v| v.abs()).sum::<f32>(), 1.0, "{normal:?}");
        let centre_to_vertex: Vec<f32> = mesh.positions[3 * i..3 * i + 3].iter().map(|p| p - 0.5).collect();
        let outwards: f32 = normal.iter().zip(&centre_to_vertex).map(|(n, c)| n * c).sum();
        assert!(outwards > 0.0);
    }
    // Axis normals have no decimals.
    assert_eq!(mesh.header.normals.decimals, 0);
}

#[test]
fn soft_normals_of_a_sphere() {
    // A UV sphere of radius 1, with seams (positions repeated) and poles.
    let (rings, segments) = (8, 12);
    let mut text = String::new();
    let index = |ring: usize, segment: usize| 1 + ring * (segments + 1) + segment;
    for ring in 0..=rings {
        for segment in 0..=segments {
            let theta = std::f64::consts::PI * ring as f64 / rings as f64;
            let phi = 2.0 * std::f64::consts::PI * segment as f64 / segments as f64;
            let p = [theta.sin() * phi.cos(), theta.sin() * phi.sin(), theta.cos()];
            text += &format!("v {:.6} {:.6} {:.6}\n", p[0], p[1], p[2]);
        }
    }
    for ring in 0..rings {
        for segment in 0..segments {
            let (a, b, c, d) = (index(ring, segment), index(ring + 1, segment), index(ring + 1, segment + 1), index(ring, segment + 1));
            text += &format!("f {a} {b} {c} {d}\n");
        }
    }
    let converted = one(&text, &Settings { normals: Normals::Soft, ..Settings::default() });
    let mesh = read(&converted);
    for (normal, position) in mesh.normals.chunks(3).zip(mesh.positions.chunks(3)) {
        // Pointing away from the centre: close to the position itself.
        let error: f32 = normal.iter().zip(position).map(|(n, p)| (n - p).abs()).sum();
        assert!(error < 0.05, "normal {normal:?} at {position:?}");
    }
    assert_eq!(mesh.header.normals.decimals, 4);
}

#[test]
fn computed_normals_replace_the_objs() {
    let converted = one(TRIANGLE.replace("vn 0 0 1", "vn 0 0 -1").as_str(), &Settings { normals: Normals::Flat, ..Settings::default() });
    assert_eq!(read(&converted).normals[2], 1.0);
    assert!(converted.warnings.iter().any(|w| w.contains("replaced")));
}

#[test]
fn splits_objects() {
    let text = format!("{TRIANGLE}o Second\nf 3/3/1 2/2/1 1/1/1\no First Again\nf 1/1/1 2/2/1 3/3/1\n");
    let settings = Settings { split: true, ..Settings::default() };
    let conversion = convert(&text, &settings).unwrap();
    let names: Vec<_> = conversion.meshes.iter().map(|m| m.object.clone().unwrap()).collect();
    assert_eq!(names, ["unnamed", "Second", "First Again"]);
    assert_eq!(read(&conversion.meshes[1]).indices, [0, 1, 2]);
    assert_eq!(split_path(Path::new("out/car.fmesh"), "First Again"), Path::new("out/car_First_Again.fmesh"));

    // Without `o`, by `g`; the same name twice is one object.
    let text = format!("{TRIANGLE}g A\nf 3/3/1 2/2/1 1/1/1\ng B\nf 1/1/1 2/2/1 3/3/1\ng A\nf 1/1/1 3/3/1 2/2/1\n");
    let conversion = convert(&text, &settings).unwrap();
    let names: Vec<_> = conversion.meshes.iter().map(|m| m.object.clone().unwrap()).collect();
    assert_eq!(names, ["unnamed", "A", "B"]);
    assert_eq!(conversion.meshes[1].stats.triangles, 2);

    // Not split: everything in one.
    let conversion = convert(&text, &Settings::default()).unwrap();
    assert_eq!(conversion.meshes.len(), 1);
    assert_eq!(conversion.meshes[0].stats.triangles, 4);
}

#[test]
fn check_finds_differences() {
    let converted = one(TRIANGLE, &Settings::default());
    let mut other = convert(&TRIANGLE.replace("v 1 0 0", "v 1 0 0.001"), &Settings::default()).unwrap();
    let error = check(&converted, &other.meshes.remove(0).written.bytes).unwrap_err().to_string();
    assert!(error.contains("position z"), "{error}");
}

#[test]
fn every_encoding_reads_back() {
    for encoding in [EncodingChoice::Auto, EncodingChoice::F32, EncodingChoice::Int, EncodingChoice::Delta] {
        for compression in [Compression::Auto, Compression::On, Compression::Off] {
            for text in [TRIANGLE, CUBE] {
                one(text, &Settings { encoding, compression, normals: Normals::Soft, ..Settings::default() });
            }
        }
    }
}

#[test]
fn rejects_obj_without_faces() {
    assert!(convert("v 0 0 0\n", &Settings::default()).is_err());
}

#[test]
fn streams_are_fixed_when_possible() {
    let converted = one(TRIANGLE, &Settings::default());
    assert!(matches!(converted.mesh.positions, Stream::Fixed { decimals: 0, .. }));
}

/// `testdata/triangle.fmesh`, the example of `doc/fmesh-format.md` (also used by the engine's
/// tests), is what the converter makes of `testdata/triangle.obj` with
/// `--encoding int --compress off`.
#[test]
fn makes_the_documented_example() {
    let obj = include_str!("../../../crates/feather-fmesh/testdata/triangle.obj");
    let settings = Settings { encoding: EncodingChoice::Int, compression: Compression::Off, ..Settings::default() };
    let converted = one(obj, &settings);
    assert_eq!(converted.written.bytes, include_bytes!("../../../crates/feather-fmesh/testdata/triangle.fmesh"));
    assert_eq!(converted.written.bytes, include_bytes!("../../../crates/feather/testdata/triangle.fmesh"));
}
