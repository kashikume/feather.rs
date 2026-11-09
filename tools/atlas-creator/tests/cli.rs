//! Runs the `atlas-creator` executable and checks its exit codes and output files.

use std::env;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output};

/// A fresh directory under the system temp directory, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let path = env::temp_dir().join(format!("atlas-creator-cli-{}-{name}", process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_solid_png(path: &Path, width: u32, height: u32, rgba: [u8; 4]) {
    let file = File::create(path).unwrap();
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(&rgba.repeat((width * height) as usize))
        .unwrap();
}

/// Writes the images and an input list naming them, in `dir`.
fn write_inputs(dir: &TempDir, images: &[(&str, u32, u32, [u8; 4])]) {
    let mut list = String::new();
    for (name, width, height, rgba) in images {
        write_solid_png(&dir.join(name), *width, *height, *rgba);
        list.push_str(name);
        list.push('\n');
    }
    fs::write(dir.join("list.txt"), list).unwrap();
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_atlas-creator"))
        .args(args)
        .output()
        .unwrap()
}

/// Runs with `dir/list.txt` as input and `dir/atlas.png` / `dir/atlas.txt` as outputs.
fn run_in(dir: &TempDir, extra: &[&str]) -> Output {
    let list = dir.join("list.txt");
    let png = dir.join("atlas.png");
    let meta = dir.join("atlas.txt");
    let mut args = vec![
        "--input",
        list.to_str().unwrap(),
        "--output",
        png.to_str().unwrap(),
        "--meta",
        meta.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    run(&args)
}

#[test]
fn no_arguments_prints_usage() {
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
}

#[test]
fn missing_input_list_exits_with_2() {
    let dir = TempDir::new("missing_input_list_exits_with_2");
    assert_eq!(run_in(&dir, &[]).status.code(), Some(2));
}

#[test]
fn images_that_do_not_fit_exit_with_4() {
    let dir = TempDir::new("images_that_do_not_fit_exit_with_4");
    write_inputs(&dir, &[("a.png", 16, 16, [1, 2, 3, 255])]);
    let output = run_in(&dir, &["--width", "16", "--height", "16"]);
    assert_eq!(output.status.code(), Some(4));
    assert!(!dir.join("atlas.png").exists());
}

#[test]
fn unwritable_output_exits_with_5() {
    let dir = TempDir::new("unwritable_output_exits_with_5");
    write_inputs(&dir, &[("a.png", 4, 4, [1, 2, 3, 255])]);
    let missing_dir = dir.join("missing/atlas.png");
    let output = run(&[
        "-i",
        dir.join("list.txt").to_str().unwrap(),
        "-o",
        missing_dir.to_str().unwrap(),
        "-m",
        dir.join("atlas.txt").to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(5));
}

#[test]
fn creates_atlas_and_meta_file() {
    let dir = TempDir::new("creates_atlas_and_meta_file");
    let images = [
        ("red.png", 10, 6, [255, 0, 0, 255]),
        ("green.png", 4, 12, [0, 255, 0, 255]),
        ("blue.png", 3, 3, [0, 0, 255, 128]),
    ];
    write_inputs(&dir, &images);

    let output = run_in(&dir, &["-w", "32", "-h", "16"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");

    // The atlas has the requested size.
    let decoder = png::Decoder::new(File::open(dir.join("atlas.png")).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut atlas = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut atlas).unwrap();
    assert_eq!((info.width, info.height), (32, 16));
    assert_eq!(info.color_type, png::ColorType::Rgba);

    // Each line of the meta file is `name x y width height`; every image is in it once, with
    // its size, and the atlas holds its pixels there.
    let meta = fs::read_to_string(dir.join("atlas.txt")).unwrap();
    assert_eq!(meta.lines().count(), images.len(), "{meta}");
    for (file, width, height, rgba) in images {
        let name = file.trim_end_matches(".png");
        let line = meta
            .lines()
            .find(|l| l.split(' ').next() == Some(name))
            .unwrap_or_else(|| panic!("{name} missing from:\n{meta}"));
        let fields: Vec<usize> = line
            .split(' ')
            .skip(1)
            .map(|f| f.parse().unwrap())
            .collect();
        let [x, y, w, h] = fields[..] else {
            panic!("bad line `{line}`")
        };
        assert_eq!((w, h), (width as usize, height as usize), "{line}");
        assert!(x >= 1 && y >= 1 && x + w < 32 && y + h < 16, "{line}");

        for py in y..y + h {
            for px in x..x + w {
                let at = (py * 32 + px) * 4;
                assert_eq!(atlas[at..at + 4], rgba, "{name} at ({px}, {py})");
            }
        }
    }
}

#[test]
fn debug_rects_option_still_succeeds() {
    let dir = TempDir::new("debug_rects_option_still_succeeds");
    write_inputs(&dir, &[("a.png", 4, 4, [1, 2, 3, 255])]);
    let output = run_in(&dir, &["-w", "16", "-h", "16", "--rects", "--verbose"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(dir.join("atlas.png").exists());
    assert!(dir.join("atlas.txt").exists());
}
