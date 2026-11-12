//! Measures `.fmesh` sizes and load times on OBJ files, for `doc/fmesh-format.md`:
//!
//!     cargo run --release -p mesh-converter --example measure -- apps/testapp/resources/*.obj
//!
//! For each file: the size of the data with every encoding, deflated or not (and, for
//! reference, the automatic encoding compressed with zstd and lz4), then the time to load the
//! OBJ the way `MeshBuilderObjFile` does (tobj, then merging vertices) against the time to read
//! the `.fmesh` and build the same vertices, from memory.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use mesh_converter::{convert, Compression, EncodingChoice, Settings};

const RUNS: usize = 15;

fn fmesh(text: &str, encoding: EncodingChoice, compression: Compression) -> Vec<u8> {
    let settings = Settings { encoding, compression, ..Settings::default() };
    convert(text, &settings).unwrap().meshes.remove(0).written.bytes
}

/// The fastest of [`RUNS`] runs of `f`.
fn time(mut f: impl FnMut()) -> Duration {
    (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .min()
        .unwrap()
}

/// What `MeshBuilderObjFile::build` does, without the scene.
fn load_obj(bytes: &[u8]) -> (Vec<[f32; 8]>, Vec<u32>) {
    let (models, _) = tobj::load_obj_buf(
        &mut &bytes[..],
        &tobj::LoadOptions { triangulate: true, single_index: true, ..Default::default() },
        |_| Ok(Default::default()),
    )
    .unwrap();
    let mut unique: HashMap<[u32; 8], u32> = HashMap::new();
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for model in &models {
        let m = &model.mesh;
        for &index in &m.indices {
            let (p, t) = (3 * index as usize, 2 * index as usize);
            let vertex = [
                m.positions[p],
                m.positions[p + 1],
                m.positions[p + 2],
                m.normals[p],
                m.normals[p + 1],
                m.normals[p + 2],
                m.texcoords[t],
                1.0 - m.texcoords[t + 1],
            ];
            let index = *unique.entry(vertex.map(f32::to_bits)).or_insert_with(|| {
                vertices.push(vertex);
                vertices.len() as u32 - 1
            });
            indices.push(index);
        }
    }
    (vertices, indices)
}

/// What `MeshBuilderFmeshFile::build` does, without the scene.
fn load_fmesh(bytes: &[u8]) -> (Vec<[f32; 8]>, Vec<u32>) {
    let mesh = feather_fmesh::read(bytes).unwrap();
    let vertices = (0..mesh.vertex_count())
        .map(|i| {
            let (p, n, t) = (&mesh.positions[3 * i..], &mesh.normals[3 * i..], &mesh.uvs[2 * i..]);
            [p[0], p[1], p[2], n[0], n[1], n[2], t[0], t[1]]
        })
        .collect();
    (vertices, mesh.indices)
}

fn kib(bytes: usize) -> String {
    format!("{:.1}", bytes as f64 / 1024.0)
}

fn ms(d: Duration) -> String {
    format!("{:.2}", d.as_secs_f64() * 1000.0)
}

fn main() {
    let paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let encodings = [
        ("f32", EncodingChoice::F32),
        ("int", EncodingChoice::Int),
        ("delta", EncodingChoice::Delta),
        ("auto", EncodingChoice::Auto),
    ];

    println!("Sizes in KiB of the data (no header). \"+d\": deflated. zstd (level 19) and lz4 compress the auto encoding.\n");
    print!("| File | OBJ | OBJ+d |");
    for (name, _) in &encodings {
        print!(" {name} | {name}+d |");
    }
    println!(" zstd | lz4 |");
    println!("|---|---:|---:|{}---:|---:|", "---:|---:|".repeat(encodings.len()));

    let mut totals = vec![0usize; 4 + 2 * encodings.len()];
    let mut timings = Vec::new();
    for path in &paths {
        let text = std::fs::read_to_string(path).unwrap();
        let name = path.file_stem().unwrap().to_string_lossy().replace("_mesh", "");
        let data = |bytes: Vec<u8>| bytes[feather_fmesh::HEADER_SIZE..].to_vec();

        let mut sizes = vec![text.len(), miniz_oxide::deflate::compress_to_vec(text.as_bytes(), 9).len()];
        for (_, encoding) in &encodings {
            sizes.push(data(fmesh(&text, *encoding, Compression::Off)).len());
            sizes.push(data(fmesh(&text, *encoding, Compression::On)).len());
        }
        let auto = data(fmesh(&text, EncodingChoice::Auto, Compression::Off));
        sizes.push(zstd::bulk::compress(&auto, 19).unwrap().len());
        sizes.push(lz4_flex::compress(&auto).len());

        print!("| {name} |");
        for (total, size) in totals.iter_mut().zip(&sizes) {
            *total += size;
            print!(" {} |", kib(*size));
        }
        println!();

        // Load times, from memory.
        let obj_bytes = text.as_bytes();
        let plain = fmesh(&text, EncodingChoice::Auto, Compression::Off);
        let deflated = fmesh(&text, EncodingChoice::Auto, Compression::On);
        let int = fmesh(&text, EncodingChoice::Int, Compression::Off);
        let (obj_vertices, obj_indices) = load_obj(obj_bytes);
        let (vertices, indices) = load_fmesh(&plain);
        // The same mesh both ways (corner by corner, as zero signs may merge differently).
        assert_eq!(indices.len(), obj_indices.len(), "{name}");
        for (a, b) in indices.iter().zip(&obj_indices) {
            let (a, b) = (vertices[*a as usize], obj_vertices[*b as usize]);
            for c in 0..8 {
                // V: tobj computes 1 - v in f32, the .fmesh has the exact value: one ulp apart at most.
                let same = if c == 7 { (a[c] - b[c]).abs() <= a[c].abs().max(1.0) * f32::EPSILON } else { a[c] == b[c] };
                assert!(same, "{name}: {a:?} {b:?}");
            }
        }
        timings.push((
            name,
            time(|| drop(load_obj(obj_bytes))),
            time(|| drop(load_fmesh(&int))),
            time(|| drop(load_fmesh(&plain))),
            time(|| drop(load_fmesh(&deflated))),
        ));
    }
    print!("| **Total** |");
    for total in &totals {
        print!(" **{}** |", kib(*total));
    }
    println!();

    println!("\nLoad times in ms (fastest of {RUNS} runs, from memory): OBJ with tobj and merging vertices, .fmesh read and turned into vertices.\n");
    println!("| File | OBJ | int | auto | auto+d |");
    println!("|---|---:|---:|---:|---:|");
    let mut sum = [Duration::ZERO; 4];
    for (name, obj, int, auto, deflated) in &timings {
        println!("| {name} | {} | {} | {} | {} |", ms(*obj), ms(*int), ms(*auto), ms(*deflated));
        for (s, t) in sum.iter_mut().zip([obj, int, auto, deflated]) {
            *s += *t;
        }
    }
    println!("| **Total** | **{}** | **{}** | **{}** | **{}** |", ms(sum[0]), ms(sum[1]), ms(sum[2]), ms(sum[3]));
}
