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
}
