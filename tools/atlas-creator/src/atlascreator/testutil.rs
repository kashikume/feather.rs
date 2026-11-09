//! Helpers shared by the unit tests.

use std::env;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process;

use super::inputparser::InputParser;

/// A fresh directory under the system temp directory, removed when dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    /// `name` must be unique among the tests (it names the directory).
    pub fn new(name: &str) -> TempDir {
        let path = env::temp_dir().join(format!("atlas-creator-{}-{name}", process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Writes an 8-bit PNG with the given color type.
pub fn write_png(path: &Path, width: u32, height: u32, color: png::ColorType, data: &[u8]) {
    let file = File::create(path).unwrap();
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(data).unwrap();
}

/// Writes an RGBA PNG filled with one color.
pub fn write_solid_png(path: &Path, width: u32, height: u32, rgba: [u8; 4]) {
    let data: Vec<u8> = rgba.repeat((width * height) as usize);
    write_png(path, width, height, png::ColorType::Rgba, &data);
}

/// Reads a PNG, returning its size, color type and pixel bytes.
pub fn read_png(path: &Path) -> (u32, u32, png::ColorType, Vec<u8>) {
    let decoder = png::Decoder::new(File::open(path).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width, info.height, info.color_type, buf)
}

/// Settings for a `width` x `height` atlas whose input list is `dir/list.txt` and outputs are
/// `dir/atlas.png` and `dir/atlas.txt`.
pub fn input(dir: &TempDir, width: usize, height: usize, files: &[&str]) -> InputParser {
    InputParser {
        width,
        height,
        output_file_name: dir.join("atlas.png").to_string_lossy().into_owned(),
        meta_file_name: dir.join("atlas.txt").to_string_lossy().into_owned(),
        input_list_file_name: dir.join("list.txt"),
        verbose: false,
        debug: false,
        debug_rects: false,
        input_files: files.iter().map(PathBuf::from).collect(),
    }
}
