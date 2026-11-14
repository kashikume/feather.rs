//! Packs PNG images into one atlas image plus a `.atlas` metadata file listing where each
//! image is (`name x y width height` per line, in pixels).
//!
//! Used by the `atlas-creator` executable and, through [`create_atlas`], by build scripts.

pub mod atlascreator;

use std::fmt;
use std::path::Path;

use atlascreator::images::Images;
use atlascreator::inputparser::InputParser;
use atlascreator::rects::Rects;

/// Why an atlas couldn't be created. Each kind has its own exit code in the executable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtlasError {
    /// The `.atlist` input list can't be read.
    InputList,
    /// An image of the list can't be read.
    Images,
    /// The images don't fit in an atlas of the requested size.
    DoesNotFit,
    /// The atlas or the metadata file can't be written.
    Output,
}

impl AtlasError {
    /// Exit code of the executable for this error.
    pub fn exit_code(self) -> u8 {
        match self {
            AtlasError::InputList => 2,
            AtlasError::Images => 3,
            AtlasError::DoesNotFit => 4,
            AtlasError::Output => 5,
        }
    }
}

impl fmt::Display for AtlasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AtlasError::InputList => "cannot read the input list",
            AtlasError::Images => "cannot read the images",
            AtlasError::DoesNotFit => "the images don't fit in the atlas",
            AtlasError::Output => "cannot write the atlas",
        })
    }
}

impl std::error::Error for AtlasError {}

/// Creates the atlas described by `input`: loads its input list, packs the images and writes
/// the atlas PNG and the metadata file.
pub fn run(input: &mut InputParser) -> Result<(), AtlasError> {
    input.load_input_list().map_err(|_| AtlasError::InputList)?;

    if input.debug {
        println!("Input: {:?}", input);
    }

    let mut images = Images::new(input);
    let mut rects = Rects::new(input.width, input.height);

    images.check_images(input).map_err(|_| AtlasError::Images)?;
    images
        .find_positions(input, &mut rects)
        .map_err(|_| AtlasError::DoesNotFit)?;

    images.build_output(input);
    if input.debug_rects {
        images.draw_debug_rects(input, &rects);
    }

    if input.debug {
        println!("Rects: {:?}", rects);
    }

    images
        .save_output_png(input)
        .map_err(|_| AtlasError::Output)?;
    images.save_meta(input).map_err(|_| AtlasError::Output)?;
    Ok(())
}

/// Creates a `width` x `height` atlas from the images listed in `input_list` (a `.atlist`
/// file: one PNG per line, relative to the list), writing the atlas to `output_png` and the
/// metadata to `output_meta` (a `.atlas` file).
pub fn create_atlas(
    input_list: &Path,
    output_png: &Path,
    output_meta: &Path,
    width: usize,
    height: usize,
) -> Result<(), AtlasError> {
    let mut input = InputParser::new(input_list, output_png, output_meta, width, height);
    run(&mut input)
}
