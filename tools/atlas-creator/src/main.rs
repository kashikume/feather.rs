use std::env;
use std::process::ExitCode;

use atlas_creator::atlascreator::inputparser::InputParser;

fn main() -> ExitCode {
    let mut input: InputParser = match InputParser::from_commandline(&env::args().collect()) {
        Ok(res) => res,
        Err(_) => {
            InputParser::show_usage();
            return ExitCode::from(1);
        }
    };

    match atlas_creator::run(&mut input) {
        Ok(()) => ExitCode::from(0),
        Err(e) => ExitCode::from(e.exit_code()),
    }
}
