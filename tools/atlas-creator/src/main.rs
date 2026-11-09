use std::env;
use std::process::ExitCode;

use crate::atlascreator::images::Images;
use crate::atlascreator::inputparser::InputParser;
use crate::atlascreator::rects::Rects;

pub mod atlascreator;

fn main() -> ExitCode {
    let mut input: InputParser = match InputParser::from_commandline(&env::args().collect()) {
        Ok(res) => res,
        Err(_) => {
            InputParser::show_usage();
            return ExitCode::from(1);
        }
    };

    match input.load_input_list() {
        Err(_) => {
            return ExitCode::from(2);
        }
        Ok(_) => (),
    };

    if input.debug {
        println!("Input: {:?}", input);
    }

    let mut images = Images::new(&input);
    let mut rects = Rects::new(input.width, input.height);

    match images.check_images(&input) {
        Err(_) => {
            return ExitCode::from(3);
        }
        Ok(_) => (),
    }
    match images.find_positions(&input, &mut rects) {
        Err(_) => {
            return ExitCode::from(4);
        }
        Ok(_) => (),
    }

    images.build_output(&input);
    if input.debug_rects {
        images.draw_debug_rects(&input, &rects);
    }

    if input.debug {
        println!("Rects: {:?}", rects);
    }

    match images.save_output_png(&input) {
        Err(_) => {
            return ExitCode::from(5);
        }
        Ok(_) => (),
    }

    match images.save_meta(&input) {
        Err(_) => {
            return ExitCode::from(5);
        }
        Ok(_) => (),
    }

    ExitCode::from(0)
}
