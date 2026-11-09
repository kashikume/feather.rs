use std::fs;
use std::io::Read;
use std::path::PathBuf;

#[derive(Debug)]
pub struct InputParser {
    pub width: usize,
    pub height: usize,
    pub output_file_name: String,
    pub meta_file_name: String,
    pub input_list_file_name: PathBuf,
    pub verbose: bool,
    pub debug: bool,
    pub debug_rects: bool,
    pub input_files: Vec<PathBuf>,
}

pub struct InputParserError;

impl InputParser {
    pub fn from_commandline(args: &Vec<String>) -> Result<InputParser, InputParserError> {
        let mut output = InputParser {
            width: 512,
            height: 512,
            output_file_name: String::new(),
            meta_file_name: String::new(),
            input_list_file_name: PathBuf::new(),
            verbose: false,
            debug: false,
            debug_rects: false,
            input_files: vec![],
        };

        for i in 0..args.len() {
            if args[i].starts_with("-") {
                match args[i].as_str() {
                    "--width" | "-w" => {
                        output.width =
                            args[i + 1].parse::<usize>().map_err(|_| InputParserError)?;
                    }
                    "--height" | "-h" => {
                        output.height =
                            args[i + 1].parse::<usize>().map_err(|_| InputParserError)?;
                    }
                    "--output" | "-o" => {
                        output.output_file_name = args[i + 1].clone();
                    }
                    "--meta" | "-m" => {
                        output.meta_file_name = args[i + 1].clone();
                    }
                    "--input" | "-i" => {
                        output.input_list_file_name = PathBuf::from(args[i + 1].as_str());
                    }
                    "--debug" => {
                        output.debug = true;
                    }
                    "--verbose" => {
                        output.verbose = true;
                    }
                    "--rects" => {
                        output.debug_rects = true;
                    }
                    _ => {
                        return Err(InputParserError);
                    }
                }
            }
        }
        if output.input_list_file_name.as_os_str().is_empty()
            || output.output_file_name.is_empty()
            || output.meta_file_name.is_empty()
        {
            return Err(InputParserError);
        }
        Ok(output)
    }

    pub fn show_usage() {
        println!("Usage:");
        println!( "AtlasCreator --width 512 --height 512 --output output.png --meta output.txt --input input.txt" );
        println!(" --width [number] | -w [number] - sets width of output png file (default 512).");
        println!(
            " --height [number] | -h [number] - sets height of output png file (default 512)."
        );
        println!( " --input [filename.txt] | -i [filename.txt] - input file name. The file should contain slit of png files to be included in the atlas." );
        println!( " --output [filename.png] | -o [filename.png] - output png file name. Result atlas saved as png file." );
        println!(
            " --meta [filename.txt] | -m [filename.txt] - output meda data about created atlas."
        );
        println!(" --rects - enables debug rects in output.");
        println!(" --verbose - additinal logs.");
        println!(" --debug - debug logs.");
    }

    pub fn load_input_list(&mut self) -> Result<(), ()> {
        if self.verbose {
            println!("Loading input list...");
        }

        let mut file = match fs::File::open(self.input_list_file_name.clone()) {
            Err(why) => {
                println!("Unable to read {:?}: {}", self.input_list_file_name, why);
                return Err(());
            }
            Ok(f) => f,
        };

        let mut buf = String::new();
        match file.read_to_string(&mut buf) {
            Err(why) => {
                println!(
                    "Error while reading {:?}: {}",
                    self.input_list_file_name, why
                );
                return Err(());
            }
            Ok(_) => (),
        };

        self.input_files = buf.lines().map(PathBuf::from).collect();

        if self.verbose {
            println!("List of files to include: {:?}", self.input_files);
        }

        Ok(())
    }
}
