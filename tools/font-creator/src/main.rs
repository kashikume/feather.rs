//! The `font-creator` executable: see the library for what it does, and `--help` for its
//! options.

mod options;

use std::process::ExitCode;

use font_creator::{create_font_images, fonts};
use options::Options;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match Options::parse(&args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            Options::show_usage();
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("Error: {e}\n");
            Options::show_usage();
            return ExitCode::from(1);
        }
    };

    if options.list_fonts {
        for font in fonts::installed_fonts() {
            println!("{} ({})", font.name, font.path.display());
        }
        return ExitCode::SUCCESS;
    }

    let atlist = options.atlist_path();
    match create_font_images(
        &options.font,
        options.height,
        &options.chars,
        &options.out_dir,
        &atlist,
    ) {
        Ok(result) => {
            println!(
                "Font: {} ({})",
                result.font.name,
                result.font.path.display()
            );
            for c in result.skipped {
                println!("Skipped U+{:04X} '{c}': not in the font", c as u32);
            }
            println!(
                "Written {} images to {} and the list to {}",
                result.written,
                options.out_dir.display(),
                atlist.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(2)
        }
    }
}
