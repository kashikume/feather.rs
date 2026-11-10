use std::path::PathBuf;

use font_creator::{chars_to_render, default_chars};

#[derive(Debug)]
pub struct Options {
    /// A font name (full name like "Arial Bold", or a file name in the Windows font directory,
    /// with or without the extension) or a path to a font file.
    pub font: String,
    /// The height of every image in pixels: the distance from the font's ascent to its descent.
    pub height: u32,
    /// Where the PNG images go.
    pub out_dir: PathBuf,
    /// The `.atlist` file; `<out_dir>.atlist` when not given.
    pub atlist: Option<PathBuf>,
    /// The characters to render, sorted, without duplicates.
    pub chars: Vec<char>,
    pub list_fonts: bool,
}

impl Options {
    /// The options from the command-line arguments (without the program name), or `None` when
    /// help is asked for.
    pub fn parse(args: &[String]) -> Result<Option<Options>, String> {
        let mut options = Options {
            font: String::from("arial"),
            height: 32,
            out_dir: PathBuf::from("font"),
            atlist: None,
            chars: Vec::new(),
            list_fonts: false,
        };
        let mut chars = default_chars();

        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = || {
                args.next()
                    .cloned()
                    .ok_or_else(|| format!("missing value for {arg}"))
            };
            match arg.as_str() {
                "--font" | "-f" => options.font = value()?,
                "--height" | "-h" => {
                    let v = value()?;
                    options.height = v
                        .parse()
                        .ok()
                        .filter(|&h| h > 0)
                        .ok_or_else(|| format!("invalid height: {v}"))?;
                }
                "--out-dir" | "-o" => options.out_dir = PathBuf::from(value()?),
                "--atlist" | "-a" => options.atlist = Some(PathBuf::from(value()?)),
                "--chars" | "-c" => chars = value()?,
                "--add-chars" => chars.push_str(&value()?),
                "--list" | "-l" => options.list_fonts = true,
                "--help" => return Ok(None),
                _ => return Err(format!("invalid argument: {arg}")),
            }
        }

        options.chars = chars_to_render(&chars);
        if options.chars.is_empty() && !options.list_fonts {
            return Err(String::from("no characters to render"));
        }
        Ok(Some(options))
    }

    pub fn atlist_path(&self) -> PathBuf {
        self.atlist
            .clone()
            .unwrap_or_else(|| self.out_dir.with_extension("atlist"))
    }

    pub fn show_usage() {
        println!("Renders the characters of a font to one PNG per character and lists them in an .atlist file.");
        println!();
        println!("font-creator --font \"Segoe UI\" --height 32 --out-dir atlases/font --atlist atlases/font_atlas.atlist");
        println!();
        println!(" --font [name] | -f [name]     - font full name (\"Arial Bold\"), file name in the Windows font directory (\"arialbd\") or path to a font file. Default: arial.");
        println!(" --height [px] | -h [px]       - height of every image in pixels (font ascent to descent). Default: 32.");
        println!(" --out-dir [dir] | -o [dir]    - directory for the images, named <code point>.png (e.g. 65.png for 'A'). Default: font.");
        println!(" --atlist [file] | -a [file]   - the .atlist file listing the images. Default: <out-dir>.atlist.");
        println!(" --chars [text] | -c [text]    - the characters to render, replacing the default set (printable ASCII, Polish, German, Spanish letters, typographic quotes and common symbols).");
        println!(" --add-chars [text]            - characters to render in addition to the set.");
        println!(" --list | -l                   - list the installed fonts and exit.");
        println!(" --help                        - show this help.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Options {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        Options::parse(&args).unwrap().unwrap()
    }

    #[test]
    fn defaults() {
        let o = parse(&[]);
        assert_eq!(o.font, "arial");
        assert_eq!(o.height, 32);
        assert_eq!(o.atlist_path(), PathBuf::from("font.atlist"));
        assert!(o.chars.contains(&' '));
        assert!(o.chars.contains(&'~'));
        assert!(o.chars.contains(&'ł'));
        assert!(o.chars.contains(&'’'));
        assert!(o.chars.contains(&'\u{00A0}'));
        assert!(o.chars.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn options() {
        let o = parse(&[
            "-f",
            "Segoe UI",
            "-h",
            "48",
            "-o",
            "out/glyphs",
            "-a",
            "x.atlist",
            "-c",
            "cba",
            "--add-chars",
            "ad",
        ]);
        assert_eq!(o.font, "Segoe UI");
        assert_eq!(o.height, 48);
        assert_eq!(o.out_dir, PathBuf::from("out/glyphs"));
        assert_eq!(o.atlist_path(), PathBuf::from("x.atlist"));
        assert_eq!(o.chars, vec!['a', 'b', 'c', 'd']);
    }

    #[test]
    fn errors() {
        let parse =
            |args: &[&str]| Options::parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(parse(&["--height"]).is_err());
        assert!(parse(&["--height", "0"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
        assert!(parse(&["--chars", ""]).is_err());
        assert!(parse(&["--help"]).unwrap().is_none());
    }
}
