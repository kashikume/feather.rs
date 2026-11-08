use anyhow::{bail, Context, Result};

use super::assetsprovider::AssetsProvider;
use super::object::Object;

/// An image that textures are cut from. Pixels are stored as 8-bit RGBA, row by row.
pub struct Atlas {
    name: Option<String>,
    handle: usize,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    /// Handles (in `Scene::textures`) of all textures in this atlas, including the full texture.
    textures: Vec<usize>,
    /// Handle of the texture covering the whole atlas, if there is one.
    full_texture: Option<usize>,
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
    /// Loads an atlas from the PNG asset `asset` (e.g. `"textures/wall.png"`). Any PNG color
    /// type and bit depth is converted to 8-bit RGBA. When `name` is `None` or empty, the
    /// asset's file name (e.g. `wall.png`) is used.
    pub fn new_from_png(
        assets: &dyn AssetsProvider,
        asset: &str,
        name: Option<String>,
    ) -> Result<Self> {
        let name = match name {
            Some(name) if !name.is_empty() => name,
            _ => asset.rsplit('/').next().unwrap_or(asset).to_string(),
        };

        let bytes = assets.get_asset(asset)?;
        Self::new_from_png_bytes(&bytes, Some(name))
            .with_context(|| format!("Cannot read PNG asset {asset}"))
    }

    /// Decodes an atlas from PNG data in memory. Any PNG color type and bit depth is converted
    /// to 8-bit RGBA.
    pub fn new_from_png_bytes(png: &[u8], name: Option<String>) -> Result<Self> {
        let (width, height, pixels) = read_png_rgba(png)?;
        Ok(Self::from_rgba(name, width, height, pixels))
    }

    /// Creates an atlas from 8-bit RGBA pixels (`width * height * 4` bytes).
    pub(crate) fn from_rgba(
        name: Option<String>,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Self {
        debug_assert_eq!(pixels.len(), width as usize * height as usize * 4);
        Self {
            name,
            handle: usize::MAX,
            width,
            height,
            pixels,
            textures: Vec::new(),
            full_texture: None,
        }
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

    /// Adds a texture handle. Adding the same handle twice keeps one copy.
    pub fn add_texture(&mut self, texture: usize) {
        if !self.textures.contains(&texture) {
            self.textures.push(texture);
        }
    }

    /// Removes a texture handle. If it was the full texture, the atlas no longer has one.
    pub fn remove_texture(&mut self, texture: usize) {
        self.textures.retain(|&t| t != texture);
        if self.full_texture == Some(texture) {
            self.full_texture = None;
        }
    }

    pub fn get_textures(&self) -> &[usize] {
        &self.textures
    }

    /// Sets the texture covering the whole atlas. It is also added to the texture list.
    pub fn set_full_texture(&mut self, texture: usize) {
        self.add_texture(texture);
        self.full_texture = Some(texture);
    }

    pub fn get_full_texture(&self) -> Option<usize> {
        self.full_texture
    }
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

    /// A 512x512 PNG in resources/.
    const TEST_PNG: &str = "demo_001_cuboid_atlas.png";
    use crate::feather::assetsprovider::tests::{test_resources, MemoryAssetsProvider};

    /// 2x1 atlas with a red and a green pixel.
    fn test_atlas(name: Option<&str>) -> Atlas {
        Atlas::from_rgba(
            name.map(str::to_string),
            2,
            1,
            vec![255, 0, 0, 255, 0, 255, 0, 255],
        )
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

    // --- loading ---

    #[test]
    fn new_from_png_loads_resource() {
        let atlas = Atlas::new_from_png(&*test_resources(), TEST_PNG, None).unwrap();
        assert_eq!(atlas.get_width(), 512);
        assert_eq!(atlas.get_height(), 512);
        assert_eq!(atlas.get_pixels().len(), 512 * 512 * 4);
        assert_eq!(atlas.get_handle(), usize::MAX);
        assert!(atlas.get_textures().is_empty());
        assert_eq!(atlas.get_full_texture(), None);
    }

    #[test]
    fn name_defaults_to_file_name() {
        let atlas = Atlas::new_from_png(&*test_resources(), TEST_PNG, None).unwrap();
        assert_eq!(atlas.get_name(), Some(TEST_PNG.to_string()));
    }

    #[test]
    fn empty_name_defaults_to_file_name() {
        let atlas = Atlas::new_from_png(&*test_resources(), TEST_PNG, Some(String::new())).unwrap();
        assert_eq!(atlas.get_name(), Some(TEST_PNG.to_string()));
    }

    #[test]
    fn explicit_name_is_kept() {
        let atlas =
            Atlas::new_from_png(&*test_resources(), TEST_PNG, Some("bricks".to_string())).unwrap();
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
        assert!(Atlas::new_from_png(&*test_resources(), "does_not_exist.png", None).is_err());
    }

    #[test]
    fn non_png_asset_fails() {
        let mesh = "demo_000_viking_room_mesh.obj";
        assert!(Atlas::new_from_png(&*test_resources(), mesh, None).is_err());
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

        let atlas = Atlas::new_from_png(&assets, "sub/dir/tiny.png", None).unwrap();
        assert_eq!(atlas.get_name(), Some("tiny.png".to_string()));
    }

    #[test]
    fn missing_asset_error_names_the_asset() {
        let err = Atlas::new_from_png(&*test_resources(), "does_not_exist.png", None)
            .err()
            .unwrap();
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

    // --- textures ---

    #[test]
    fn from_rgba_keeps_data() {
        let atlas = test_atlas(Some("atlas"));
        assert_eq!(atlas.get_name(), Some("atlas".to_string()));
        assert_eq!((atlas.get_width(), atlas.get_height()), (2, 1));
        assert_eq!(atlas.get_pixels(), &[255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    fn add_and_remove_textures() {
        let mut atlas = test_atlas(None);
        atlas.add_texture(1);
        atlas.add_texture(2);
        atlas.add_texture(1);
        assert_eq!(atlas.get_textures(), &[1, 2]);

        atlas.remove_texture(1);
        assert_eq!(atlas.get_textures(), &[2]);

        atlas.remove_texture(42); // missing handle is ignored
        assert_eq!(atlas.get_textures(), &[2]);
    }

    #[test]
    fn full_texture_is_in_texture_list() {
        let mut atlas = test_atlas(None);
        atlas.add_texture(1);
        atlas.set_full_texture(2);

        assert_eq!(atlas.get_full_texture(), Some(2));
        assert_eq!(atlas.get_textures(), &[1, 2]);
    }

    #[test]
    fn set_full_texture_to_existing_texture_does_not_duplicate() {
        let mut atlas = test_atlas(None);
        atlas.add_texture(1);
        atlas.set_full_texture(1);
        assert_eq!(atlas.get_textures(), &[1]);
    }

    #[test]
    fn replacing_full_texture_keeps_old_one_in_list() {
        let mut atlas = test_atlas(None);
        atlas.set_full_texture(1);
        atlas.set_full_texture(2);

        assert_eq!(atlas.get_full_texture(), Some(2));
        assert_eq!(atlas.get_textures(), &[1, 2]);
    }

    #[test]
    fn removing_full_texture_clears_it() {
        let mut atlas = test_atlas(None);
        atlas.set_full_texture(1);
        atlas.remove_texture(1);

        assert_eq!(atlas.get_full_texture(), None);
        assert!(atlas.get_textures().is_empty());
    }
}
