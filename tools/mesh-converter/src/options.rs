use std::path::{Path, PathBuf};

use mesh_converter::{Compression, EncodingChoice, Normals, Settings};

#[derive(Debug, PartialEq)]
pub struct Options {
    pub inputs: Vec<PathBuf>,
    /// The output for a single input; `None` to put it next to the input (or in `out_dir`).
    pub output: Option<PathBuf>,
    pub out_dir: Option<PathBuf>,
    pub settings: Settings,
    pub check: bool,
    pub stats: bool,
    pub verbose: bool,
    pub quiet: bool,
}

impl Options {
    /// The options from the command-line arguments (without the program name), or `None` when
    /// help is asked for.
    pub fn parse(args: &[String]) -> Result<Option<Options>, String> {
        let mut options = Options {
            inputs: Vec::new(),
            output: None,
            out_dir: None,
            settings: Settings::default(),
            check: false,
            stats: false,
            verbose: false,
            quiet: false,
        };
        let mut positional = Vec::new();
        let (mut flat, mut soft) = (false, false);

        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = || args.next().cloned().ok_or_else(|| format!("missing value for {arg}"));
            match arg.as_str() {
                "--out-dir" | "-o" => options.out_dir = Some(PathBuf::from(value()?)),
                "--split" | "-s" => options.settings.split = true,
                "--compress" | "-c" => {
                    options.settings.compression = match value()?.as_str() {
                        "auto" => Compression::Auto,
                        "on" => Compression::On,
                        "off" => Compression::Off,
                        v => return Err(format!("invalid --compress: {v} (auto, on or off)")),
                    }
                }
                "--encoding" | "-e" => {
                    options.settings.encoding = match value()?.as_str() {
                        "auto" => EncodingChoice::Auto,
                        "f32" => EncodingChoice::F32,
                        "int" => EncodingChoice::Int,
                        "delta" => EncodingChoice::Delta,
                        v => return Err(format!("invalid --encoding: {v} (auto, f32, int or delta)")),
                    }
                }
                "--max-decimals" => {
                    let v = value()?;
                    options.settings.max_decimals = v
                        .parse()
                        .ok()
                        .filter(|&d| d <= feather_fmesh::MAX_DECIMALS as u32)
                        .ok_or_else(|| format!("invalid --max-decimals: {v} (0 to {})", feather_fmesh::MAX_DECIMALS))?;
                }
                "--flat-normals" => flat = true,
                "--soft-normals" => soft = true,
                "--no-flip-v" => options.settings.flip_v = false,
                "--check" => options.check = true,
                "--stats" => options.stats = true,
                "--verbose" | "-v" => options.verbose = true,
                "--quiet" | "-q" => options.quiet = true,
                "--help" | "-h" => return Ok(None),
                _ if arg.starts_with('-') && arg.len() > 1 => return Err(format!("invalid argument: {arg}")),
                _ => positional.push(PathBuf::from(arg)),
            }
        }

        options.settings.normals = match (flat, soft) {
            (true, true) => return Err("--flat-normals and --soft-normals can't be used together".into()),
            (true, false) => Normals::Flat,
            (false, true) => Normals::Soft,
            (false, false) => Normals::FromObj,
        };
        if options.verbose && options.quiet {
            return Err("--verbose and --quiet can't be used together".into());
        }

        match (options.out_dir.is_some(), positional.len()) {
            (_, 0) => return Err("no input file".into()),
            (true, _) => options.inputs = positional,
            (false, 1) => options.inputs = positional,
            (false, 2) => {
                options.output = positional.pop();
                options.inputs = positional;
            }
            (false, _) => return Err("several input files need --out-dir".into()),
        }
        Ok(Some(options))
    }

    /// Where `input` is converted to: the output given, or `<input name>.fmesh` in `out_dir`
    /// or next to the input.
    pub fn output_for(&self, input: &Path) -> PathBuf {
        if let Some(output) = &self.output {
            return output.clone();
        }
        let name = Path::new(input.file_name().unwrap_or_default()).with_extension("fmesh");
        match &self.out_dir {
            Some(dir) => dir.join(name),
            None => input.with_extension("fmesh"),
        }
    }

    pub fn show_usage() {
        println!("Converts Wavefront OBJ files into .fmesh files, feather's binary meshes.");
        println!("Every number is kept exactly: each stream is stored as integers with as many decimals as its");
        println!("most precise number (or as f32 when that isn't possible).");
        println!();
        println!("mesh-converter [options] <input.obj> [output.fmesh]");
        println!("mesh-converter [options] --out-dir <dir> <input.obj>...");
        println!();
        println!("mesh-converter resources/car.obj --soft-normals --stats");
        println!();
        println!(" --out-dir [dir] | -o [dir]    - write <dir>/<input name>.fmesh. Needed with several inputs. Default: next to the input.");
        println!(" --split | -s                  - one file per o (or, without any, g) object: <output name>_<object>.fmesh.");
        println!("                                 Default: all objects merged into one mesh.");
        println!(" --compress [auto|on|off] | -c - deflate the data: auto keeps it only if it saves 10% or more. Default: auto.");
        println!(" --encoding [auto|f32|int|delta] | -e");
        println!("                               - how values are stored: auto the smallest per stream, f32 floats, int fixed-width");
        println!("                                 integers, delta differences as varints. Default: auto.");
        println!(" --max-decimals [n]            - the most decimals a stream may have to be stored as integers. Default: 9.");
        println!(" --flat-normals                - compute normals: each triangle's own (sharp edges).");
        println!(" --soft-normals                - compute normals: averaged around each position (smooth), within smoothing groups.");
        println!(" --no-flip-v                   - keep V as in the OBJ instead of storing 1 - v.");
        println!(" --check                       - read each file back and compare every value with the OBJ.");
        println!(" --stats                       - print counts, precision, encodings and sizes.");
        println!(" --verbose | -v                - also print skipped lines and split polygons.");
        println!(" --quiet | -q                  - print only errors.");
        println!(" --help | -h                   - print this.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &str) -> Result<Option<Options>, String> {
        let args: Vec<String> = args.split_whitespace().map(String::from).collect();
        Options::parse(&args)
    }

    #[test]
    fn defaults() {
        let options = parse("car.obj").unwrap().unwrap();
        assert_eq!(options.inputs, [PathBuf::from("car.obj")]);
        assert_eq!(options.settings, Settings::default());
        assert_eq!(options.output_for(Path::new("dir/car.obj")), PathBuf::from("dir/car.fmesh"));
        assert!(!options.check && !options.stats && !options.verbose && !options.quiet);
    }

    #[test]
    fn every_option() {
        let options = parse("-s -c on -e delta --max-decimals 12 --soft-normals --no-flip-v --check --stats -v a.obj b.fmesh")
            .unwrap()
            .unwrap();
        assert_eq!(
            options.settings,
            Settings {
                split: true,
                encoding: EncodingChoice::Delta,
                compression: Compression::On,
                max_decimals: 12,
                normals: Normals::Soft,
                flip_v: false,
            }
        );
        assert!(options.check && options.stats && options.verbose);
        assert_eq!(options.output_for(Path::new("a.obj")), PathBuf::from("b.fmesh"));
        assert_eq!(parse("--flat-normals -q a.obj").unwrap().unwrap().settings.normals, Normals::Flat);
    }

    #[test]
    fn out_dir() {
        let options = parse("a.obj --out-dir out x/b.obj").unwrap().unwrap();
        assert_eq!(options.inputs.len(), 2);
        assert_eq!(options.output_for(Path::new("x/b.obj")), PathBuf::from("out/b.fmesh"));
    }

    #[test]
    fn errors() {
        for args in [
            "",
            "a.obj b.obj c.obj",
            "--flat-normals --soft-normals a.obj",
            "-v -q a.obj",
            "--compress maybe a.obj",
            "--encoding f16 a.obj",
            "--max-decimals 19 a.obj",
            "--max-decimals x a.obj",
            "--precision 10000 a.obj",
            "a.obj --out-dir",
        ] {
            assert!(parse(args).is_err(), "{args}");
        }
        assert_eq!(parse("--help").unwrap(), None);
    }
}
