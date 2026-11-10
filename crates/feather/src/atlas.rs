use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use super::assetsprovider::AssetsProvider;
use super::object::Object;
use super::texture::Texture;

/// A named rectangle of an atlas, in pixels, as listed in its metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// An image that textures are cut from. Pixels are stored as 8-bit RGBA, row by row.
///
/// The optional metadata (a `.atlas` file, as written by the atlas creator tool) names
/// rectangles of the image; `get_texture` turns one into a `Texture`.
pub struct Atlas {
    name: Option<String>,
    handle: usize,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    /// Named rectangles, from the metadata.
    regions: HashMap<String, AtlasRegion>,
}

impl Object for Atlas {
    fn get_name(&self) -> Option<String> {
        self.name.clone()
    }

    fn set_handle(&mut self, handle: usize) {
        self.handle = handle;
    }

    fn get_handle(&self) -> usize {
        self.handle
    }
}

impl Atlas {
    /// Loads an atlas from the PNG asset `bitmap` (e.g. `"textures/wall.png"`) and, when given,
    /// the metadata asset `metadata` (e.g. `"textures/wall.atlas"`, see `parse_metadata`). Any
    /// PNG color type and bit depth is converted to 8-bit RGBA. When `name` is `None` or empty,
    /// the bitmap's file name (e.g. `wall.png`) is used.
    pub fn new_from_png(
        assets: &dyn AssetsProvider,
        bitmap: &str,
        metadata: Option<&str>,
        name: Option<String>,
    ) -> Result<Self> {
        let name = match name {
            Some(name) if !name.is_empty() => name,
            _ => bitmap.rsplit('/').next().unwrap_or(bitmap).to_string(),
        };

        let bytes = assets.get_asset(bitmap)?;
        let mut atlas = Self::new_from_png_bytes(&bytes, Some(name))
            .with_context(|| format!("Cannot read PNG asset {bitmap}"))?;
        if let Some(metadata) = metadata {
            (|| {
                let bytes = assets.get_asset(metadata)?;
                let text = std::str::from_utf8(&bytes).context("not UTF-8")?;
                atlas.set_metadata(text)
            })()
            .with_context(|| format!("Cannot read atlas metadata {metadata}"))?;
        }
        Ok(atlas)
    }

    /// Decodes an atlas from PNG data in memory. Any PNG color type and bit depth is converted
    /// to 8-bit RGBA. The atlas has no metadata; see `set_metadata`.
    pub fn new_from_png_bytes(png: &[u8], name: Option<String>) -> Result<Self> {
        let (width, height, pixels) = read_png_rgba(png)?;
        Ok(Self::from_rgba(name, width, height, pixels))
    }

    /// Creates an atlas from 8-bit RGBA pixels (`width * height * 4` bytes, row by row from
    /// the top), e.g. a texture generated in code. The atlas has no metadata; see
    /// `set_metadata`. Panics if `pixels` has the wrong length.
    pub fn from_rgba(
        name: Option<String>,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Self {
        assert_eq!(pixels.len(), width as usize * height as usize * 4);
        Self {
            name,
            handle: usize::MAX,
            width,
            height,
            pixels,
            regions: HashMap::new(),
        }
    }

    /// Replaces the named rectangles with those in `text` (see `parse_metadata`). Fails, keeping
    /// the old ones, if the text is invalid or a rectangle doesn't fit in the atlas.
    pub fn set_metadata(&mut self, text: &str) -> Result<()> {
        let regions = parse_metadata(text)?;
        for (name, r) in &regions {
            let fits = r.x as u64 + r.width as u64 <= self.width as u64
                && r.y as u64 + r.height as u64 <= self.height as u64;
            if !fits {
                bail!(
                    "`{name}` ({} {} {} {}) is outside the {}x{} atlas",
                    r.x,
                    r.y,
                    r.width,
                    r.height,
                    self.width,
                    self.height
                );
            }
        }
        self.regions = regions;
        Ok(())
    }

    pub fn get_width(&self) -> u32 {
        self.width
    }

    pub fn get_height(&self) -> u32 {
        self.height
    }

    /// 8-bit RGBA pixels, row by row, `width * height * 4` bytes.
    pub fn get_pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// The texture named `name` in the metadata, or `None` if there is no such name.
    ///
    /// The texture refers to the atlas by handle, so get it once the atlas is in a scene
    /// (`Scene::get_atlas`).
    pub fn get_texture(&self, name: &str) -> Option<Texture> {
        let r = self.regions.get(name)?;
        let (w, h) = (self.width as f32, self.height as f32);
        Some(Texture::new(
            self.handle,
            r.x as f32 / w,
            (r.x + r.width) as f32 / w,
            r.y as f32 / h,
            (r.y + r.height) as f32 / h,
        ))
    }

    /// A texture covering the whole atlas. See `get_texture` about the handle.
    pub fn get_full_texture(&self) -> Texture {
        Texture::full(self.handle)
    }

    /// Whether the metadata names a texture `name`.
    pub fn has_texture(&self, name: &str) -> bool {
        self.regions.contains_key(name)
    }

    /// Names of the textures in the metadata, sorted.
    pub fn get_texture_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.regions.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// The rectangle, in pixels, of texture `name`.
    pub fn get_region(&self, name: &str) -> Option<AtlasRegion> {
        self.regions.get(name).copied()
    }
}

/// Parses atlas metadata (a `.atlas` file): one texture per line, `name x y width height`,
/// in pixels from the top-left corner of the image. The name is everything before the last
/// four numbers, so it may contain spaces. Empty lines are skipped; names must be unique.
pub fn parse_metadata(text: &str) -> Result<HashMap<String, AtlasRegion>> {
    let mut regions = HashMap::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (name, region) = parse_metadata_line(line)
            .with_context(|| format!("line {}: `{line}`", index + 1))?;
        if regions.insert(name.to_string(), region).is_some() {
            bail!("line {}: texture `{name}` is listed twice", index + 1);
        }
    }
    Ok(regions)
}

fn parse_metadata_line(line: &str) -> Result<(&str, AtlasRegion)> {
    let mut parts = line.rsplitn(5, char::is_whitespace);
    let mut number = || -> Result<u32> {
        let part = parts.next().unwrap_or("");
        part.parse().with_context(|| {
            format!("expected `name x y width height`, `{part}` is not a number")
        })
    };
    let (height, width, y, x) = (number()?, number()?, number()?, number()?);
    let name = parts.next().map(str::trim_end).unwrap_or("");
    if name.is_empty() {
        bail!("expected `name x y width height`, the name is missing");
    }
    Ok((
        name,
        AtlasRegion {
            x,
            y,
            width,
            height,
        },
    ))
}

/// Decodes a PNG into 8-bit RGBA pixels. Returns `(width, height, pixels)`.
fn read_png_rgba(png: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(png);
    // Palette and low bit depths expand to 8 bits, 16-bit channels are cut down to 8.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;

    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer)?;
    buffer.truncate(info.buffer_size());

    let pixels = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => buffer.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buffer
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => bail!("Indexed PNG was not expanded"),
    };

    Ok((info.width, info.height, pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 512x512 PNG in testdata/.
    const TEST_PNG: &str = "atlas_512.png";
    use crate::assetsprovider::tests::{test_resources, MemoryAssetsProvider};

    /// 2x1 atlas with a red and a green pixel.
    fn test_atlas(name: Option<&str>) -> Atlas {
        Atlas::from_rgba(
            name.map(str::to_string),
            2,
            1,
            vec![255, 0, 0, 255, 0, 255, 0, 255],
        )
    }

    /// 4x2 atlas with no metadata.
    fn atlas_4x2() -> Atlas {
        Atlas::from_rgba(None, 4, 2, vec![0; 4 * 2 * 4])
    }

    /// Encodes a PNG in memory.
    fn write_png(
        width: u32,
        height: u32,
        color: png::ColorType,
        depth: png::BitDepth,
        data: &[u8],
        palette: Option<Vec<u8>>,
    ) -> Vec<u8> {
        let mut png = Vec::new();
        let mut encoder = png::Encoder::new(&mut png, width, height);
        encoder.set_color(color);
        encoder.set_depth(depth);
        if let Some(palette) = palette {
            encoder.set_palette(palette);
        }
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(data).unwrap();
        writer.finish().unwrap();
        png
    }

    fn load(asset: &str, name: Option<String>) -> Result<Atlas> {
        Atlas::new_from_png(&*test_resources(), asset, None, name)
    }

    // --- loading ---

    #[test]
    fn new_from_png_loads_resource() {
        let atlas = load(TEST_PNG, None).unwrap();
        assert_eq!(atlas.get_width(), 512);
        assert_eq!(atlas.get_height(), 512);
        assert_eq!(atlas.get_pixels().len(), 512 * 512 * 4);
        assert_eq!(atlas.get_handle(), usize::MAX);
        assert!(atlas.get_texture_names().is_empty());
    }

    #[test]
    fn name_defaults_to_file_name() {
        let atlas = load(TEST_PNG, None).unwrap();
        assert_eq!(atlas.get_name(), Some(TEST_PNG.to_string()));
    }

    #[test]
    fn empty_name_defaults_to_file_name() {
        let atlas = load(TEST_PNG, Some(String::new())).unwrap();
        assert_eq!(atlas.get_name(), Some(TEST_PNG.to_string()));
    }

    #[test]
    fn explicit_name_is_kept() {
        let atlas = load(TEST_PNG, Some("bricks".to_string())).unwrap();
        assert_eq!(atlas.get_name(), Some("bricks".to_string()));
    }

    #[test]
    fn rgba_png_is_loaded_as_is() {
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let png = write_png(
            2,
            1,
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            &data,
            None,
        );
        let atlas = Atlas::new_from_png_bytes(&png, None).unwrap();
        assert_eq!((atlas.get_width(), atlas.get_height()), (2, 1));
        assert_eq!(atlas.get_pixels(), &data);
    }

    #[test]
    fn rgb_png_gets_opaque_alpha() {
        let png = write_png(
            2,
            1,
            png::ColorType::Rgb,
            png::BitDepth::Eight,
            &[10, 20, 30, 40, 50, 60],
            None,
        );
        let atlas = Atlas::new_from_png_bytes(&png, None).unwrap();
        assert_eq!(atlas.get_pixels(), &[10, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn grayscale_png_is_converted() {
        let png = write_png(
            2,
            1,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &[0, 200],
            None,
        );
        let atlas = Atlas::new_from_png_bytes(&png, None).unwrap();
        assert_eq!(atlas.get_pixels(), &[0, 0, 0, 255, 200, 200, 200, 255]);
    }

    #[test]
    fn grayscale_alpha_png_is_converted() {
        let png = write_png(
            1,
            1,
            png::ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            &[100, 50],
            None,
        );
        let atlas = Atlas::new_from_png_bytes(&png, None).unwrap();
        assert_eq!(atlas.get_pixels(), &[100, 100, 100, 50]);
    }

    #[test]
    fn sixteen_bit_png_is_reduced_to_eight_bits() {
        // One RGBA pixel, 16 bits per channel (big-endian); the high byte is kept.
        let png = write_png(
            1,
            1,
            png::ColorType::Rgba,
            png::BitDepth::Sixteen,
            &[0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xFF, 0xFF],
            None,
        );
        let atlas = Atlas::new_from_png_bytes(&png, None).unwrap();
        assert_eq!(atlas.get_pixels(), &[0x12, 0x56, 0x9A, 0xFF]);
    }

    #[test]
    fn indexed_png_is_expanded() {
        let palette = vec![255, 0, 0, 0, 0, 255]; // red, blue
        let png = write_png(
            2,
            1,
            png::ColorType::Indexed,
            png::BitDepth::Eight,
            &[1, 0],
            Some(palette),
        );
        let atlas = Atlas::new_from_png_bytes(&png, None).unwrap();
        assert_eq!(atlas.get_pixels(), &[0, 0, 255, 255, 255, 0, 0, 255]);
    }

    #[test]
    fn missing_file_fails() {
        assert!(load("does_not_exist.png", None).is_err());
    }

    #[test]
    fn non_png_asset_fails() {
        assert!(load("quad.obj", None).is_err());
        assert!(Atlas::new_from_png_bytes(b"not a png", None).is_err());
    }

    #[test]
    fn name_defaults_to_last_path_segment() {
        let png = write_png(
            1,
            1,
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            &[1, 2, 3, 4],
            None,
        );
        let mut assets = MemoryAssetsProvider::default();
        assets.assets.insert("sub/dir/tiny.png".to_string(), png);

        let atlas = Atlas::new_from_png(&assets, "sub/dir/tiny.png", None, None).unwrap();
        assert_eq!(atlas.get_name(), Some("tiny.png".to_string()));
    }

    #[test]
    fn missing_asset_error_names_the_asset() {
        let err = load("does_not_exist.png", None).err().unwrap();
        assert!(format!("{err:#}").contains("does_not_exist.png"));
    }

    #[test]
    fn new_from_png_bytes_keeps_name() {
        let png = write_png(
            1,
            1,
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            &[1, 2, 3, 4],
            None,
        );
        let atlas = Atlas::new_from_png_bytes(&png, Some("tiny".to_string())).unwrap();
        assert_eq!(atlas.get_name(), Some("tiny".to_string()));
        assert_eq!(Atlas::new_from_png_bytes(&png, None).unwrap().get_name(), None);
    }

    #[test]
    fn from_rgba_keeps_data() {
        let atlas = test_atlas(Some("atlas"));
        assert_eq!(atlas.get_name(), Some("atlas".to_string()));
        assert_eq!((atlas.get_width(), atlas.get_height()), (2, 1));
        assert_eq!(atlas.get_pixels(), &[255, 0, 0, 255, 0, 255, 0, 255]);
        assert!(atlas.get_texture_names().is_empty());
    }

    // --- metadata and textures ---

    #[test]
    fn parse_metadata_reads_name_and_rectangle() {
        let regions = parse_metadata("a 1 2 3 4\r\n\n  b c 0 0 10 20  \n").unwrap();
        assert_eq!(regions.len(), 2);
        assert_eq!(
            regions["a"],
            AtlasRegion {
                x: 1,
                y: 2,
                width: 3,
                height: 4
            }
        );
        // Names may contain spaces.
        assert_eq!(
            regions["b c"],
            AtlasRegion {
                x: 0,
                y: 0,
                width: 10,
                height: 20
            }
        );
        assert!(parse_metadata("").unwrap().is_empty());
    }

    #[test]
    fn parse_metadata_rejects_bad_lines() {
        for text in [
            "a 1 2 3",
            "1 2 3 4",
            "a 1 2 3 x",
            "a -1 2 3 4",
            "a 1 2 3 4\na 0 0 1 1",
        ] {
            assert!(parse_metadata(text).is_err(), "accepted `{text}`");
        }
        let err = parse_metadata("ok 0 0 1 1\nbad 1 2").unwrap_err();
        assert!(format!("{err:#}").contains("line 2"), "{err:#}");
    }

    #[test]
    fn get_texture_converts_pixels_to_uv() {
        let mut atlas = atlas_4x2();
        atlas.set_handle(7);
        atlas.set_metadata("left 0 0 2 2\ncorner 3 1 1 1\n").unwrap();

        assert_eq!(
            atlas.get_texture("left"),
            Some(Texture::new(7, 0.0, 0.5, 0.0, 1.0))
        );
        assert_eq!(
            atlas.get_texture("corner"),
            Some(Texture::new(7, 0.75, 1.0, 0.5, 1.0))
        );
        assert_eq!(atlas.get_texture("missing"), None);
        assert!(atlas.has_texture("left"));
        assert!(!atlas.has_texture("missing"));
        assert_eq!(atlas.get_texture_names(), ["corner", "left"]);
        assert_eq!(
            atlas.get_region("corner"),
            Some(AtlasRegion {
                x: 3,
                y: 1,
                width: 1,
                height: 1
            })
        );
    }

    #[test]
    fn full_texture_covers_the_atlas() {
        let mut atlas = atlas_4x2();
        atlas.set_handle(2);
        assert_eq!(atlas.get_full_texture(), Texture::full(2));
    }

    #[test]
    fn metadata_outside_the_atlas_is_rejected() {
        let mut atlas = atlas_4x2();
        atlas.set_metadata("a 0 0 1 1").unwrap();
        assert!(atlas.set_metadata("b 3 0 2 1").is_err());
        assert!(atlas.set_metadata("b 0 1 1 2").is_err());
        // The old metadata stays.
        assert_eq!(atlas.get_texture_names(), ["a"]);
    }

    #[test]
    fn new_from_png_reads_metadata() {
        let png = write_png(
            4,
            2,
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            &[0; 4 * 2 * 4],
            None,
        );
        let mut assets = MemoryAssetsProvider::default();
        assets.assets.insert("a.png".to_string(), png);
        assets
            .assets
            .insert("a.atlas".to_string(), b"one 0 0 4 1\ntwo 0 1 4 1\n".to_vec());
        assets
            .assets
            .insert("bad.atlas".to_string(), b"one 0 0 5 1".to_vec());

        let atlas = Atlas::new_from_png(&assets, "a.png", Some("a.atlas"), None).unwrap();
        assert_eq!(atlas.get_texture_names(), ["one", "two"]);

        let err = Atlas::new_from_png(&assets, "a.png", Some("bad.atlas"), None)
            .err()
            .unwrap();
        assert!(format!("{err:#}").contains("bad.atlas"), "{err:#}");
        assert!(Atlas::new_from_png(&assets, "a.png", Some("missing.atlas"), None).is_err());
    }
}
