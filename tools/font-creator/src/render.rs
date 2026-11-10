//! Rendering characters to images.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};

pub struct Renderer<'a> {
    font: FontRef<'a>,
    scale: PxScale,
    height: u32,
    /// The baseline's distance from the top of the image, in pixels.
    baseline: f32,
}

/// A character's image: white, with the glyph's coverage in the alpha channel.
pub struct Glyph {
    pub width: u32,
    pub height: u32,
    /// RGBA, 8 bits per channel, row by row from the top.
    pub pixels: Vec<u8>,
}

impl<'a> Renderer<'a> {
    /// A renderer of the face `index` of the font file `data`, making images `height` pixels
    /// high, from the font's ascent to its descent.
    pub fn new(data: &'a [u8], index: u32, height: u32) -> Result<Renderer<'a>, String> {
        let font = FontRef::try_from_slice_and_index(data, index)
            .map_err(|e| format!("invalid font: {e}"))?;
        // A PxScale makes ascent - descent the given number of pixels.
        let scale = PxScale::from(height as f32);
        let baseline = font.as_scaled(scale).ascent().round();
        Ok(Renderer {
            font,
            scale,
            height,
            baseline,
        })
    }

    /// The image of `c`, as wide as the character's advance (so that images put side by side
    /// make the text), or `None` when the font has no glyph for it. Ink outside the image (e.g.
    /// the overhang of italic letters) is cut off.
    pub fn render(&self, c: char) -> Option<Glyph> {
        let id = self.font.glyph_id(c);
        if id.0 == 0 {
            return None;
        }
        let advance = self.font.as_scaled(self.scale).h_advance(id);
        let width = (advance.round() as u32).max(1);
        let height = self.height;
        let mut pixels = [255, 255, 255, 0].repeat((width * height) as usize);

        let glyph = id.with_scale_and_position(self.scale, point(0.0, self.baseline));
        if let Some(outline) = self.font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|x, y, coverage| {
                let x = bounds.min.x as i64 + x as i64;
                let y = bounds.min.y as i64 + y as i64;
                if (0..width as i64).contains(&x) && (0..height as i64).contains(&y) {
                    let alpha = &mut pixels[((y * width as i64 + x) * 4 + 3) as usize];
                    *alpha = (*alpha).max((coverage.clamp(0.0, 1.0) * 255.0).round() as u8);
                }
            });
        }
        Some(Glyph {
            width,
            height,
            pixels,
        })
    }
}

impl Glyph {
    pub fn save(&self, path: &Path) -> Result<(), png::EncodingError> {
        let file = File::create(path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&self.pixels)?;
        writer.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Arial, when the machine has it.
    fn arial() -> Option<Vec<u8>> {
        let windir = std::env::var("WINDIR").ok()?;
        std::fs::read(Path::new(&windir).join("Fonts\\arial.ttf")).ok()
    }

    #[test]
    fn renders_glyphs_of_the_given_height() {
        let Some(data) = arial() else { return };
        let renderer = Renderer::new(&data, 0, 32).unwrap();

        let a = renderer.render('A').unwrap();
        assert_eq!(a.height, 32);
        assert_eq!(a.pixels.len(), (a.width * a.height * 4) as usize);
        assert!(a.pixels.chunks(4).any(|p| p[3] == 255));

        let space = renderer.render(' ').unwrap();
        assert!(space.width > 0);
        assert!(space.pixels.chunks(4).all(|p| p[3] == 0));

        let i = renderer.render('i').unwrap();
        let w = renderer.render('W').unwrap();
        assert!(i.width < w.width);
    }

    #[test]
    fn missing_glyphs_are_none() {
        let Some(data) = arial() else { return };
        let renderer = Renderer::new(&data, 0, 32).unwrap();
        assert!(renderer.render('\u{E000}').is_none());
    }
}
