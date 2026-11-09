use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

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
    /// Options for creating a `width` x `height` atlas from the `.atlist` file `input_list`,
    /// written to `output_png` and `output_meta`. The input list is not read yet.
    pub fn new(
        input_list: &Path,
        output_png: &Path,
        output_meta: &Path,
        width: usize,
        height: usize,
    ) -> InputParser {
        InputParser {
            width,
            height,
            output_file_name: output_png.to_string_lossy().into_owned(),
            meta_file_name: output_meta.to_string_lossy().into_owned(),
            input_list_file_name: input_list.to_path_buf(),
            verbose: false,
            debug: false,
            debug_rects: false,
            input_files: vec![],
        }
    }

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
        println!( "AtlasCreator --width 512 --height 512 --output sprites.png --meta sprites.atlas --input sprites.atlist" );
        println!(" --width [number] | -w [number] - sets width of output png file (default 512).");
        println!(
            " --height [number] | -h [number] - sets height of output png file (default 512)."
        );
        println!( " --input [filename.atlist] | -i [filename.atlist] - input file name. The .atlist file lists the png files to include in the atlas, one per line, relative to the .atlist file." );
        println!( " --output [filename.png] | -o [filename.png] - output png file name. Result atlas saved as png file." );
        println!(
            " --meta [filename.atlas] | -m [filename.atlas] - output .atlas file with the name, position and size of each image in the atlas."
        );
        println!(" --rects - enables debug rects in output.");
        println!(" --verbose - additional logs.");
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

        self.input_files = buf
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect();

        if self.verbose {
            println!("List of files to include: {:?}", self.input_files);
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::InputParser;
    use crate::atlascreator::testutil::TempDir;

    /// Parses `args` as given after the program name.
    fn parse(args: &[&str]) -> Option<InputParser> {
        let mut all = vec!["atlas-creator".to_string()];
        all.extend(args.iter().map(|a| a.to_string()));
        InputParser::from_commandline(&all).ok()
    }

    const REQUIRED: &[&str] = &[
        "--input",
        "in.atlist",
        "--output",
        "out.png",
        "--meta",
        "out.atlas",
    ];

    #[test]
    fn required_options_and_defaults() {
        let input = parse(REQUIRED).unwrap();
        assert_eq!(input.input_list_file_name, PathBuf::from("in.atlist"));
        assert_eq!(input.output_file_name, "out.png");
        assert_eq!(input.meta_file_name, "out.atlas");
        assert_eq!((input.width, input.height), (512, 512));
        assert!(!input.verbose);
        assert!(!input.debug);
        assert!(!input.debug_rects);
        assert!(input.input_files.is_empty());
    }

    #[test]
    fn long_options() {
        let mut args = REQUIRED.to_vec();
        args.extend([
            "--width",
            "256",
            "--height",
            "128",
            "--verbose",
            "--debug",
            "--rects",
        ]);
        let input = parse(&args).unwrap();
        assert_eq!((input.width, input.height), (256, 128));
        assert!(input.verbose);
        assert!(input.debug);
        assert!(input.debug_rects);
    }

    #[test]
    fn short_options() {
        let input = parse(&[
            "-i", "a.atlist", "-o", "b.png", "-m", "c.atlas", "-w", "64", "-h", "32",
        ])
        .unwrap();
        assert_eq!(input.input_list_file_name, PathBuf::from("a.atlist"));
        assert_eq!(input.output_file_name, "b.png");
        assert_eq!(input.meta_file_name, "c.atlas");
        assert_eq!((input.width, input.height), (64, 32));
    }

    #[test]
    fn options_in_any_order() {
        let input = parse(&[
            "--meta", "m.atlas", "-w", "8", "--output", "o.png", "-i", "l.atlist",
        ])
        .unwrap();
        assert_eq!(input.meta_file_name, "m.atlas");
        assert_eq!(input.output_file_name, "o.png");
        assert_eq!(input.input_list_file_name, PathBuf::from("l.atlist"));
        assert_eq!(input.width, 8);
    }

    #[test]
    fn each_required_option_is_required() {
        for missing in ["--input", "--output", "--meta"] {
            let pos = REQUIRED.iter().position(|a| *a == missing).unwrap();
            let mut args = REQUIRED.to_vec();
            args.drain(pos..pos + 2);
            assert!(parse(&args).is_none(), "accepted without {missing}");
        }
        assert!(parse(&[]).is_none());
    }

    #[test]
    fn unknown_option_fails() {
        let mut args = REQUIRED.to_vec();
        args.push("--nope");
        assert!(parse(&args).is_none());
    }

    #[test]
    fn invalid_size_fails() {
        for (option, value) in [("--width", "abc"), ("--height", "-5"), ("-w", "1.5")] {
            let mut args = REQUIRED.to_vec();
            args.extend([option, value]);
            assert!(parse(&args).is_none(), "accepted {option} {value}");
        }
    }

    #[test]
    fn load_input_list_reads_one_file_per_line() {
        let dir = TempDir::new("load_input_list_reads_one_file_per_line");
        let list = dir.join("list.atlist");
        fs::write(&list, "a.png\r\nsub/b.png\n\n  \nc.png\n").unwrap();

        let mut input = parse(REQUIRED).unwrap();
        input.input_list_file_name = list;
        input.load_input_list().unwrap();

        let expected: Vec<PathBuf> = ["a.png", "sub/b.png", "c.png"]
            .iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(input.input_files, expected);
    }

    #[test]
    fn new_sets_files_and_size() {
        let input = InputParser::new(
            Path::new("in/list.atlist"),
            Path::new("out/a.png"),
            Path::new("out/a.atlas"),
            256,
            128,
        );
        assert_eq!(input.input_list_file_name, PathBuf::from("in/list.atlist"));
        assert_eq!(input.output_file_name, "out/a.png");
        assert_eq!(input.meta_file_name, "out/a.atlas");
        assert_eq!((input.width, input.height), (256, 128));
        assert!(!input.verbose && !input.debug && !input.debug_rects);
        assert!(input.input_files.is_empty());
    }

    #[test]
    fn load_input_list_of_missing_file_fails() {
        let dir = TempDir::new("load_input_list_of_missing_file_fails");
        let mut input = parse(REQUIRED).unwrap();
        input.input_list_file_name = dir.join("does_not_exist.atlist");
        assert!(input.load_input_list().is_err());
    }
}
