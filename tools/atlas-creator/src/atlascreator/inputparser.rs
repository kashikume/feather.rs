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

#[cfg(test)]
mod test {
    use std::fs;
    use std::path::PathBuf;

    use super::InputParser;
    use crate::atlascreator::testutil::TempDir;

    /// Parses `args` as given after the program name.
    fn parse(args: &[&str]) -> Option<InputParser> {
        let mut all = vec!["atlas-creator".to_string()];
        all.extend(args.iter().map(|a| a.to_string()));
        InputParser::from_commandline(&all).ok()
    }

    const REQUIRED: &[&str] = &[
        "--input", "in.txt", "--output", "out.png", "--meta", "out.txt",
    ];

    #[test]
    fn required_options_and_defaults() {
        let input = parse(REQUIRED).unwrap();
        assert_eq!(input.input_list_file_name, PathBuf::from("in.txt"));
        assert_eq!(input.output_file_name, "out.png");
        assert_eq!(input.meta_file_name, "out.txt");
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
            "-i", "a.txt", "-o", "b.png", "-m", "c.txt", "-w", "64", "-h", "32",
        ])
        .unwrap();
        assert_eq!(input.input_list_file_name, PathBuf::from("a.txt"));
        assert_eq!(input.output_file_name, "b.png");
        assert_eq!(input.meta_file_name, "c.txt");
        assert_eq!((input.width, input.height), (64, 32));
    }

    #[test]
    fn options_in_any_order() {
        let input = parse(&[
            "--meta", "m.txt", "-w", "8", "--output", "o.png", "-i", "l.txt",
        ])
        .unwrap();
        assert_eq!(input.meta_file_name, "m.txt");
        assert_eq!(input.output_file_name, "o.png");
        assert_eq!(input.input_list_file_name, PathBuf::from("l.txt"));
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
        let list = dir.join("list.txt");
        fs::write(&list, "a.png\r\nsub/b.png\nc.png\n").unwrap();

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
    fn load_input_list_of_missing_file_fails() {
        let dir = TempDir::new("load_input_list_of_missing_file_fails");
        let mut input = parse(REQUIRED).unwrap();
        input.input_list_file_name = dir.join("does_not_exist.txt");
        assert!(input.load_input_list().is_err());
    }
}
