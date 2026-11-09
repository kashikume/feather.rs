use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use super::image::Image;
use super::inputparser::InputParser;
use super::rect::Rect;
use super::rects::Rects;

#[derive(Debug)]
pub struct Images {
    images: Vec<Image>,
    output: Vec<u8>,
}

impl Images {
    pub fn new(input: &InputParser) -> Images {
        return Images {
            images: vec![],
            output: vec![0; (input.width * input.height * 4) as usize],
        };
    }

    pub fn check_images(&mut self, input: &InputParser) -> Result<(), ()> {
        let path = input.input_list_file_name.parent().unwrap();
        for file in input.input_files.iter() {
            let png_file_name = path.join(file);

            let decoder = png::Decoder::new(File::open(png_file_name.clone()).unwrap());
            let mut reader = decoder.read_info().unwrap();
            let mut buf = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut buf).unwrap();

            let img = Image {
                file_name: png_file_name,
                name: String::from(file.file_stem().unwrap().to_str().unwrap()),
                rect: Rect::from_wh(info.width as usize, info.height as usize),
            };

            self.images.push(img);
        }

        self.images
            .sort_by(|a, b| b.rect.area().partial_cmp(&a.rect.area()).unwrap());
        Ok(())
    }

    // keeps 1 pixel empty frame around every image
    pub fn find_positions(&mut self, input: &InputParser, rects: &mut Rects) -> Result<(), ()> {
        for i in self.images.iter_mut() {
            if input.verbose {
                println!("Fitting: {:?}", i.file_name);
            }
            let rect = match rects.find_best(i.rect.width() + 2, i.rect.height() + 2) {
                Some(r) => r,
                None => {
                    println!("Unable to fit image");
                    return Err(());
                }
            };
            i.rect.move_to(rect.x1 + 1, rect.y1 + 1);
            let mut rc_use = i.rect.clone();
            rc_use.expand(1);
            rects.use_rect(rc_use);
        }
        Ok(())
    }

    pub fn build_output(&mut self, input: &InputParser) {
        for i in self.images.iter() {
            if input.verbose {
                println!("Adding image: {:?}", i.file_name);
            }
            let decoder = png::Decoder::new(File::open(i.file_name.as_path()).unwrap());
            let mut reader = decoder.read_info().unwrap();
            let mut buf = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut buf).unwrap();
            let bytes = &buf[..info.buffer_size()];
            if input.debug {
                println!("Image: {:?}", i);
                println!("Info: {:?}", info);
            }
            match (info.color_type, info.bit_depth) {
                (png::ColorType::Rgba, png::BitDepth::Eight) => {
                    for y in i.rect.y1..i.rect.y2 {
                        let y2 = y - i.rect.y1;
                        let output_range = (y * input.width + i.rect.x1) * 4
                            ..(y * input.width + i.rect.x1 + i.rect.width()) * 4;
                        let input_range =
                            (y2 * info.width as usize * 4)..((y2 + 1) * info.width as usize * 4);
                        self.output[output_range].clone_from_slice(&bytes[input_range]);
                    }
                },
                (png::ColorType::Rgb, png::BitDepth::Eight) => {
                    for y in i.rect.y1..i.rect.y2 {
                        let y2 = y - i.rect.y1;
                        for x in i.rect.x1..i.rect.x2 {
                            let x2 = x - i.rect.x1;
                            self.output[y*input.width*4 + x*4 + 0] = bytes[y2*info.width as usize*3 + x2*3 + 0];
                            self.output[y*input.width*4 + x*4 + 1] = bytes[y2*info.width as usize*3 + x2*3 + 1];
                            self.output[y*input.width*4 + x*4 + 2] = bytes[y2*info.width as usize*3 + x2*3 + 2];
                            self.output[y*input.width*4 + x*4 + 3] = 255;
                        }
                    }
                },
                (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => {
                    for y in i.rect.y1..i.rect.y2 {
                        let y2 = y - i.rect.y1;
                        for x in i.rect.x1..i.rect.x2 {
                            let x2 = x - i.rect.x1;
                            self.output[y*input.width*4 + x*4 + 0] = bytes[y2*info.width as usize*2 + x2*2 + 0];
                            self.output[y*input.width*4 + x*4 + 1] = bytes[y2*info.width as usize*2 + x2*2 + 0];
                            self.output[y*input.width*4 + x*4 + 2] = bytes[y2*info.width as usize*2 + x2*2 + 0];
                            self.output[y*input.width*4 + x*4 + 3] = bytes[y2*info.width as usize*2 + x2*2 + 1];
                        }
                    }
                },
                (png::ColorType::Grayscale, png::BitDepth::Eight) => {
                    for y in i.rect.y1..i.rect.y2 {
                        let y2 = y - i.rect.y1;
                        for x in i.rect.x1..i.rect.x2 {
                            let x2 = x - i.rect.x1;
                            self.output[y*input.width*4 + x*4 + 0] = bytes[y2*info.width as usize + x2];
                            self.output[y*input.width*4 + x*4 + 1] = bytes[y2*info.width as usize + x2];
                            self.output[y*input.width*4 + x*4 + 2] = bytes[y2*info.width as usize + x2];
                            self.output[y*input.width*4 + x*4 + 3] = 255;
                        }
                    }
                },
                _ => {
                    println!("Unsupported color type {:?}, {:?}.", info.color_type, info.bit_depth);
                    panic!("Unsupported color type.");
                }
            }
            Images::expand_borders(&mut self.output, &input, &i.rect);
        }
    }

    pub fn save_output_png(&self, input: &InputParser) -> Result<(), ()> {
        let path = Path::new(&input.output_file_name);
        let file = match File::create(path) {
            Err(_) => {
                return Err(());
            }
            Ok(f) => f,
        };
        let ref mut w = BufWriter::new(file);

        let mut encoder = png::Encoder::new(w, input.width as u32, input.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = match encoder.write_header() {
            Err(_) => {
                return Err(());
            }
            Ok(w) => w,
        };

        match writer.write_image_data(&self.output) {
            Err(_) => {
                return Err(());
            }
            Ok(_) => (),
        };

        Ok(())
    }

    pub fn save_meta(&self, input: &InputParser) -> std::io::Result<()> {
        if input.verbose {
            print!("Saving meta file: {:?}", input.meta_file_name);
        };

        let file = File::create(&input.meta_file_name)?;
        let mut writer = BufWriter::new(file);
        for i in self.images.iter() {
            write!(
                &mut writer,
                "{} {} {} {} {}\n",
                i.name,
                i.rect.x1,
                i.rect.y1,
                i.rect.width(),
                i.rect.height()
            )?;
        }
        Ok(())
    }

    fn expand_borders(output: &mut Vec<u8>, input: &InputParser, r: &Rect) {
        for x in r.x1..r.x2 {
            output[(r.y1 - 1) * input.width * 4 + x * 4 + 0] =
                output[r.y1 * input.width * 4 + x * 4 + 0];
            output[(r.y1 - 1) * input.width * 4 + x * 4 + 1] =
                output[r.y1 * input.width * 4 + x * 4 + 1];
            output[(r.y1 - 1) * input.width * 4 + x * 4 + 2] =
                output[r.y1 * input.width * 4 + x * 4 + 2];
            output[(r.y1 - 1) * input.width * 4 + x * 4 + 3] =
                output[r.y1 * input.width * 4 + x * 4 + 3];

            output[(r.y2) * input.width * 4 + x * 4 + 0] =
                output[(r.y2 - 1) * input.width * 4 + x * 4 + 0];
            output[(r.y2) * input.width * 4 + x * 4 + 1] =
                output[(r.y2 - 1) * input.width * 4 + x * 4 + 1];
            output[(r.y2) * input.width * 4 + x * 4 + 2] =
                output[(r.y2 - 1) * input.width * 4 + x * 4 + 2];
            output[(r.y2) * input.width * 4 + x * 4 + 3] =
                output[(r.y2 - 1) * input.width * 4 + x * 4 + 3];
        }
        for y in r.y1..r.y2 {
            output[y * input.width * 4 + (r.x1 - 1) * 4 + 0] =
                output[y * input.width * 4 + r.x1 * 4 + 0];
            output[y * input.width * 4 + (r.x1 - 1) * 4 + 1] =
                output[y * input.width * 4 + r.x1 * 4 + 1];
            output[y * input.width * 4 + (r.x1 - 1) * 4 + 2] =
                output[y * input.width * 4 + r.x1 * 4 + 2];
            output[y * input.width * 4 + (r.x1 - 1) * 4 + 3] =
                output[y * input.width * 4 + r.x1 * 4 + 3];

            output[y * input.width * 4 + (r.x2) * 4 + 0] =
                output[y * input.width * 4 + (r.x2 - 1) * 4 + 0];
            output[y * input.width * 4 + (r.x2) * 4 + 1] =
                output[y * input.width * 4 + (r.x2 - 1) * 4 + 1];
            output[y * input.width * 4 + (r.x2) * 4 + 2] =
                output[y * input.width * 4 + (r.x2 - 1) * 4 + 2];
            output[y * input.width * 4 + (r.x2) * 4 + 3] =
                output[y * input.width * 4 + (r.x2 - 1) * 4 + 3];
        }
    }

    pub fn draw_debug_rects(&mut self, input: &InputParser, rects: &Rects) {
        if input.verbose {
            println!("Drawing debug rects. ({})", rects.len());
        }
        for r in rects.all_rects() {
            for x in r.x1..r.x2 {
                self.output[r.y1 * input.width * 4 + x * 4 + 0] = 128;
                self.output[r.y1 * input.width * 4 + x * 4 + 1] = 128;
                self.output[r.y1 * input.width * 4 + x * 4 + 2] = 128;
                self.output[r.y1 * input.width * 4 + x * 4 + 3] = 128;

                self.output[(r.y2 - 1) * input.width * 4 + x * 4 + 0] = 128;
                self.output[(r.y2 - 1) * input.width * 4 + x * 4 + 1] = 128;
                self.output[(r.y2 - 1) * input.width * 4 + x * 4 + 2] = 128;
                self.output[(r.y2 - 1) * input.width * 4 + x * 4 + 3] = 128;
            }
            for y in r.y1..r.y2 {
                self.output[y * input.width * 4 + r.x1 * 4 + 0] = 128;
                self.output[y * input.width * 4 + r.x1 * 4 + 1] = 128;
                self.output[y * input.width * 4 + r.x1 * 4 + 2] = 128;
                self.output[y * input.width * 4 + r.x1 * 4 + 3] = 128;

                self.output[y * input.width * 4 + (r.x2 - 1) * 4 + 0] = 128;
                self.output[y * input.width * 4 + (r.x2 - 1) * 4 + 1] = 128;
                self.output[y * input.width * 4 + (r.x2 - 1) * 4 + 2] = 128;
                self.output[y * input.width * 4 + (r.x2 - 1) * 4 + 3] = 128;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use std::fs;

    use crate::atlascreator::testutil::{input, read_png, write_png, write_solid_png, TempDir};
    use crate::atlascreator::{image::Image, inputparser::InputParser, rect::Rect, rects::Rects};

    use super::Images;

    #[test]
    fn test_fit_100() {
        assert_eq!(helper_fit_n(100), Ok(()));
    }

    #[test]
    fn test_does_not_fit_101() {
        assert_eq!(helper_fit_n(101), Err(()));
    }

    fn helper_fit_n(n: usize) -> Result<(), ()> {
        let input = InputParser {
            width: 100,
            height: 100,
            output_file_name: String::from("filename.txt"),
            meta_file_name: String::from("filename.txt"),
            input_list_file_name: PathBuf::from("filename.txt"),
            verbose: false,
            debug: false,
            debug_rects: false,
            input_files: vec![],
        };

        let mut images = Images::new(&input);
        for _ in 0..n {
            images.images.push(Image {
                file_name: PathBuf::from("filename.txt"),
                name: String::from("name"),
                rect: Rect::from_wh(8, 8),
            });
        }
        let mut rects = Rects::new(input.width, input.height);
        images.find_positions(&input, &mut rects)
    }

    /// Loads, places and draws `files` (already written to `dir`) into a `width` x `height`
    /// atlas.
    fn build(dir: &TempDir, width: usize, height: usize, files: &[&str]) -> (InputParser, Images) {
        let input = input(dir, width, height, files);
        let mut images = Images::new(&input);
        let mut rects = Rects::new(width, height);
        images.check_images(&input).unwrap();
        images.find_positions(&input, &mut rects).unwrap();
        images.build_output(&input);
        (input, images)
    }

    fn pixel(images: &Images, input: &InputParser, x: usize, y: usize) -> [u8; 4] {
        let at = (y * input.width + x) * 4;
        images.output[at..at + 4].try_into().unwrap()
    }

    fn image<'a>(images: &'a Images, name: &str) -> &'a Image {
        images.images.iter().find(|i| i.name == name).unwrap()
    }

    #[test]
    fn new_allocates_a_transparent_rgba_output() {
        let dir = TempDir::new("new_allocates_a_transparent_rgba_output");
        let images = Images::new(&input(&dir, 7, 3, &[]));
        assert_eq!(images.output, vec![0; 7 * 3 * 4]);
        assert!(images.images.is_empty());
    }

    #[test]
    fn check_images_reads_sizes_and_sorts_by_area() {
        let dir = TempDir::new("check_images_reads_sizes_and_sorts_by_area");
        write_solid_png(&dir.join("small.png"), 2, 3, [1, 1, 1, 255]);
        write_solid_png(&dir.join("big.png"), 4, 4, [2, 2, 2, 255]);
        write_solid_png(&dir.join("wide.png"), 8, 1, [3, 3, 3, 255]);

        let input = input(&dir, 64, 64, &["small.png", "big.png", "wide.png"]);
        let mut images = Images::new(&input);
        images.check_images(&input).unwrap();

        let found: Vec<(&str, usize, usize)> = images
            .images
            .iter()
            .map(|i| (i.name.as_str(), i.rect.width(), i.rect.height()))
            .collect();
        assert_eq!(found, vec![("big", 4, 4), ("wide", 8, 1), ("small", 2, 3)]);
        // Files are relative to the directory of the input list.
        assert_eq!(image(&images, "big").file_name, dir.join("big.png"));
    }

    #[test]
    fn find_positions_keeps_a_one_pixel_gap() {
        let dir = TempDir::new("find_positions_keeps_a_one_pixel_gap");
        let input = input(&dir, 40, 40, &[]);
        let mut images = Images::new(&input);
        let sizes = [(10, 10), (7, 12), (20, 3), (5, 5), (1, 1), (12, 7)];
        for (i, (w, h)) in sizes.into_iter().enumerate() {
            images.images.push(Image {
                file_name: PathBuf::from(format!("{i}.png")),
                name: i.to_string(),
                rect: Rect::from_wh(w, h),
            });
        }
        let mut rects = Rects::new(input.width, input.height);
        images.find_positions(&input, &mut rects).unwrap();

        for (i, a) in images.images.iter().enumerate() {
            // The 1 pixel frame around each image is inside the atlas...
            assert!(a.rect.x1 >= 1 && a.rect.y1 >= 1, "{a:?}");
            assert!(a.rect.x2 + 1 <= 40 && a.rect.y2 + 1 <= 40, "{a:?}");
            // ... and belongs to that image only.
            let mut framed = a.rect.clone();
            framed.expand(1);
            for b in &images.images[i + 1..] {
                let mut other = b.rect.clone();
                other.expand(1);
                assert!(!framed.overlap(&other), "{a:?} and {b:?} share pixels");
            }
        }
    }

    #[test]
    fn image_filling_the_atlas_minus_frame_fits() {
        let dir = TempDir::new("image_filling_the_atlas_minus_frame_fits");
        write_solid_png(&dir.join("a.png"), 8, 6, [9, 9, 9, 255]);
        let (_, images) = build(&dir, 10, 8, &["a.png"]);
        let expected = Rect {
            x1: 1,
            y1: 1,
            x2: 9,
            y2: 7,
        };
        assert_eq!(image(&images, "a").rect, expected);
    }

    #[test]
    fn image_without_room_for_frame_does_not_fit() {
        let dir = TempDir::new("image_without_room_for_frame_does_not_fit");
        write_solid_png(&dir.join("a.png"), 10, 8, [9, 9, 9, 255]);
        let input = input(&dir, 10, 8, &["a.png"]);
        let mut images = Images::new(&input);
        images.check_images(&input).unwrap();
        assert!(images
            .find_positions(&input, &mut Rects::new(10, 8))
            .is_err());
    }

    #[test]
    fn build_output_copies_rgba_pixels() {
        let dir = TempDir::new("build_output_copies_rgba_pixels");
        #[rustfmt::skip]
        let data = [
            10, 11, 12, 13,   20, 21, 22, 23,   30, 31, 32, 33,
            40, 41, 42, 43,   50, 51, 52, 53,   60, 61, 62, 63,
        ];
        write_png(&dir.join("a.png"), 3, 2, png::ColorType::Rgba, &data);
        let (input, images) = build(&dir, 8, 8, &["a.png"]);

        let r = &image(&images, "a").rect;
        for y in 0..2 {
            for x in 0..3 {
                let at = (y * 3 + x) * 4;
                assert_eq!(pixel(&images, &input, r.x1 + x, r.y1 + y), data[at..at + 4]);
            }
        }
    }

    #[test]
    fn build_output_repeats_edge_pixels_into_the_frame() {
        let dir = TempDir::new("build_output_repeats_edge_pixels_into_the_frame");
        #[rustfmt::skip]
        let data = [
            1, 1, 1, 255,   2, 2, 2, 255,
            3, 3, 3, 255,   4, 4, 4, 255,
        ];
        write_png(&dir.join("a.png"), 2, 2, png::ColorType::Rgba, &data);
        let (input, images) = build(&dir, 6, 6, &["a.png"]);
        let r = image(&images, "a").rect.clone();
        assert_eq!((r.x1, r.y1), (1, 1));
        let p = |x, y| pixel(&images, &input, x, y)[0];

        // Above and below.
        assert_eq!([p(1, 0), p(2, 0)], [1, 2]);
        assert_eq!([p(1, 3), p(2, 3)], [3, 4]);
        // Left and right.
        assert_eq!([p(0, 1), p(0, 2)], [1, 3]);
        assert_eq!([p(3, 1), p(3, 2)], [2, 4]);
        // Corners of the frame and everything outside it stay transparent.
        for (x, y) in [(0, 0), (3, 0), (0, 3), (3, 3), (4, 1), (1, 4), (5, 5)] {
            assert_eq!(pixel(&images, &input, x, y), [0, 0, 0, 0], "({x}, {y})");
        }
    }

    #[test]
    fn build_output_converts_rgb_and_grayscale() {
        let dir = TempDir::new("build_output_converts_rgb_and_grayscale");
        write_png(
            &dir.join("rgb.png"),
            1,
            1,
            png::ColorType::Rgb,
            &[10, 20, 30],
        );
        write_png(
            &dir.join("gray.png"),
            1,
            1,
            png::ColorType::Grayscale,
            &[40],
        );
        write_png(
            &dir.join("graya.png"),
            1,
            1,
            png::ColorType::GrayscaleAlpha,
            &[50, 60],
        );
        let (input, images) = build(&dir, 9, 3, &["rgb.png", "gray.png", "graya.png"]);

        let at = |name| {
            let r = &image(&images, name).rect;
            pixel(&images, &input, r.x1, r.y1)
        };
        assert_eq!(at("rgb"), [10, 20, 30, 255]);
        assert_eq!(at("gray"), [40, 40, 40, 255]);
        assert_eq!(at("graya"), [50, 50, 50, 60]);
    }

    #[test]
    fn save_output_png_writes_the_atlas() {
        let dir = TempDir::new("save_output_png_writes_the_atlas");
        write_solid_png(&dir.join("a.png"), 2, 3, [7, 8, 9, 200]);
        let (input, images) = build(&dir, 6, 5, &["a.png"]);
        images.save_output_png(&input).unwrap();

        let (width, height, color, data) = read_png(&dir.join("atlas.png"));
        assert_eq!((width, height), (6, 5));
        assert_eq!(color, png::ColorType::Rgba);
        assert_eq!(data, images.output);
    }

    #[test]
    fn save_output_png_to_missing_directory_fails() {
        let dir = TempDir::new("save_output_png_to_missing_directory_fails");
        let mut input = input(&dir, 4, 4, &[]);
        let missing = dir.join("no/such/dir/atlas.png");
        input.output_file_name = missing.to_string_lossy().into_owned();
        assert!(Images::new(&input).save_output_png(&input).is_err());
    }

    #[test]
    fn save_meta_lists_name_position_and_size() {
        let dir = TempDir::new("save_meta_lists_name_position_and_size");
        write_solid_png(&dir.join("big.png"), 4, 3, [1, 1, 1, 255]);
        write_solid_png(&dir.join("small.png"), 2, 1, [2, 2, 2, 255]);
        let (input, images) = build(&dir, 16, 16, &["small.png", "big.png"]);
        images.save_meta(&input).unwrap();

        let meta = fs::read_to_string(dir.join("atlas.txt")).unwrap();
        let expected: String = images
            .images
            .iter()
            .map(|i| {
                let r = &i.rect;
                format!(
                    "{} {} {} {} {}\n",
                    i.name,
                    r.x1,
                    r.y1,
                    r.width(),
                    r.height()
                )
            })
            .collect();
        assert_eq!(meta, expected);
        // Largest first, sizes as in the source files.
        let lines: Vec<&str> = meta.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines[0].starts_with("big ") && lines[0].ends_with(" 4 3"),
            "{meta}"
        );
        assert!(
            lines[1].starts_with("small ") && lines[1].ends_with(" 2 1"),
            "{meta}"
        );
    }

    #[test]
    fn draw_debug_rects_outlines_free_rects() {
        let dir = TempDir::new("draw_debug_rects_outlines_free_rects");
        let input = input(&dir, 5, 4, &[]);
        let mut images = Images::new(&input);
        images.draw_debug_rects(&input, &Rects::new(5, 4));

        for y in 0..4 {
            for x in 0..5 {
                let edge = x == 0 || y == 0 || x == 4 || y == 3;
                let expected = if edge { [128; 4] } else { [0; 4] };
                assert_eq!(pixel(&images, &input, x, y), expected, "({x}, {y})");
            }
        }
    }
}
