//! The `mesh-converter` executable: see the library for what it does, and `--help` for its
//! options.

mod options;

use std::path::Path;
use std::process::ExitCode;

use mesh_converter::{convert_file, Conversion, Converted};
use options::Options;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match Options::parse(&args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            Options::show_usage();
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("Error: {e}\n");
            Options::show_usage();
            return ExitCode::from(1);
        }
    };

    let mut failed = 0;
    for input in &options.inputs {
        let output = options.output_for(input);
        match convert_file(input, &output, &options.settings, options.check) {
            Ok((conversion, paths)) => report(&options, input, &conversion, &paths),
            Err(e) => {
                eprintln!("error: {}: {e:#}", input.display());
                failed += 1;
            }
        }
    }
    if failed > 0 {
        if options.inputs.len() > 1 {
            eprintln!("{failed} of {} files failed", options.inputs.len());
        }
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn report(options: &Options, input: &Path, conversion: &Conversion, paths: &[std::path::PathBuf]) {
    let obj_size = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    if options.verbose {
        for (line, text) in &conversion.skipped {
            println!("{}: line {line} skipped: {text}", input.display());
        }
    }
    for (converted, path) in conversion.meshes.iter().zip(paths) {
        if !options.quiet {
            for warning in &converted.warnings {
                eprintln!("warning: {}: {warning}", path.display());
            }
        }
        if options.verbose {
            for note in &converted.notes {
                println!("{}: {note}", input.display());
            }
        }
        if !options.quiet {
            let size = converted.written.bytes.len();
            println!(
                "{} -> {}: {} triangles, {} vertices, {} bytes ({:.1}% of the OBJ){}",
                input.display(),
                path.display(),
                converted.stats.triangles,
                converted.stats.vertices,
                size,
                100.0 * size as f64 / obj_size.max(1) as f64,
                if options.check { ", checked" } else { "" }
            );
        }
        if options.stats {
            print_stats(converted, obj_size);
        }
    }
}

fn print_stats(converted: &Converted, obj_size: u64) {
    let header = &converted.written.header;
    let stats = &converted.stats;
    let stream = |s: feather_fmesh::StreamDescriptor| match s.encoding {
        feather_fmesh::Encoding::F32 => "f32".to_string(),
        e => format!("{} decimals, {}", s.decimals, e.name()),
    };
    // What the mesh takes as `Vertex` (8 floats) and `u32` indices: the engine's memory, and
    // what an `f32` file without encodings would hold.
    let raw = stats.vertices * 32 + stats.corners * 4;
    println!(
        "  triangles {}, corners {}, vertices {} (merged)",
        stats.triangles, stats.corners, stats.vertices
    );
    println!(
        "  positions: {}; normals: {}; uvs: {}; indices: {}",
        stream(header.positions),
        stream(header.normals),
        stream(header.uvs),
        header.indices.encoding.name()
    );
    println!(
        "  sizes: OBJ {obj_size}, f32 vertices + u32 indices {raw}, data {}, stored {} ({}), file {}",
        header.data_size,
        header.stored_size,
        if header.compressed() { "deflated" } else { "not compressed" },
        converted.written.bytes.len()
    );
}
