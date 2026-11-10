//! Renders the characters of a Windows font to one PNG image per character and writes an
//! `.atlist` file listing them, ready to be packed into an atlas by the atlas creator.
//!
//! Used by the `font-creator` executable and, through [`create_font_images`] and
//! [`FontSpec`], by build scripts.

pub mod fonts;
pub mod render;

use std::fs;
use std::path::{Path, PathBuf};

/// The characters rendered by default besides printable ASCII (with the space): the Polish,
/// German and Spanish letters, typographic quotes and apostrophe, dashes, ellipsis, the
/// non-breaking space and a few common symbols.
pub const DEFAULT_EXTRA_CHARS: &str = concat!(
    "ąęśćźżóńłĄĘŚĆŹŻÓŃŁ", // Polish
    "äöüÄÖÜßẞ",           // German
    "áéíñúÁÉÍÑÚ¿¡ªº",     // Spanish
    "„“”‚‘’«»‹›",         // quotes and apostrophe
    "–—…\u{00A0}",        // dashes, ellipsis, non-breaking space
    "€°•§·",              // symbols
);

/// The default characters: printable ASCII (with the space) and [`DEFAULT_EXTRA_CHARS`].
pub fn default_chars() -> String {
    let mut chars: String = (' '..='~').collect();
    chars.push_str(DEFAULT_EXTRA_CHARS);
    chars
}

/// The characters of `text` to render: without control characters, sorted, without
/// duplicates.
pub fn chars_to_render(text: &str) -> Vec<char> {
    let mut chars: Vec<char> = text.chars().filter(|c| !c.is_control()).collect();
    chars.sort();
    chars.dedup();
    chars
}

/// What [`create_font_images`] did.
pub struct FontImages {
    /// The font used.
    pub font: fonts::FontFile,
    /// How many images were written.
    pub written: usize,
    /// The characters the font has no glyph for, which got no image.
    pub skipped: Vec<char>,
}

/// Renders `chars` with the font `font` (see [`fonts::find_font`]) into images `height`
/// pixels high in `out_dir`, named `<code point>.png`, and writes the list of them to
/// `atlist`, relative to the list's directory when `out_dir` is inside it.
pub fn create_font_images(
    font: &str,
    height: u32,
    chars: &[char],
    out_dir: &Path,
    atlist: &Path,
) -> Result<FontImages, String> {
    let font_file =
        fonts::find_font(font).ok_or_else(|| format!("font '{font}' not found (try --list)"))?;
    let data = fs::read(&font_file.path)
        .map_err(|e| format!("unable to read {}: {e}", font_file.path.display()))?;
    let renderer = render::Renderer::new(&data, font_file.index, height)?;

    fs::create_dir_all(out_dir)
        .map_err(|e| format!("unable to create {}: {e}", out_dir.display()))?;

    let prefix = relative_dir(atlist, out_dir);
    let mut list = String::new();
    let (mut written, mut skipped) = (0, Vec::new());
    for &c in chars {
        let Some(glyph) = renderer.render(c) else {
            skipped.push(c);
            continue;
        };
        let file_name = format!("{}.png", c as u32);
        glyph
            .save(&out_dir.join(&file_name))
            .map_err(|e| format!("unable to write {file_name}: {e}"))?;
        list.push_str(&prefix);
        list.push_str(&file_name);
        list.push('\n');
        written += 1;
    }

    fs::write(atlist, list).map_err(|e| format!("unable to write {}: {e}", atlist.display()))?;
    Ok(FontImages {
        font: font_file,
        written,
        skipped,
    })
}

/// A font atlas to make, as described in a `.font` file: `key = value` lines, `#` starting a
/// comment line.
///
/// ```text
/// # Segoe UI, 64 pixels high, with the default characters and a few more.
/// font = Segoe UI
/// height = 64
/// add-chars = ←→
/// ```
///
/// `font` and `height` are required; `chars` replaces the default characters
/// ([`default_chars`]) and `add-chars` adds to them, as the executable's options do.
#[derive(Debug, PartialEq)]
pub struct FontSpec {
    pub font: String,
    pub height: u32,
    /// The characters to render, sorted, without duplicates.
    pub chars: Vec<char>,
}

impl FontSpec {
    pub fn parse(text: &str) -> Result<FontSpec, String> {
        let (mut font, mut height) = (None, None);
        let mut chars = default_chars();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let error = |what: &str| format!("line {}: {what}: `{line}`", index + 1);
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| error("expected `key = value`"))?;
            let value = value.trim();
            match key.trim() {
                "font" => font = Some(value.to_string()),
                "height" => {
                    height = Some(
                        value
                            .parse()
                            .ok()
                            .filter(|&h: &u32| h > 0)
                            .ok_or_else(|| error("invalid height"))?,
                    )
                }
                "chars" => chars = value.to_string(),
                "add-chars" => chars.push_str(value),
                _ => return Err(error("unknown key")),
            }
        }
        let chars = chars_to_render(&chars);
        if chars.is_empty() {
            return Err(String::from("no characters to render"));
        }
        Ok(FontSpec {
            font: font.ok_or("`font` is missing")?,
            height: height.ok_or("`height` is missing")?,
            chars,
        })
    }

    /// Reads and parses the `.font` file `path`.
    pub fn load(path: &Path) -> Result<FontSpec, String> {
        let text = fs::read_to_string(path)
            .map_err(|e| format!("unable to read {}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// The prefix (with a trailing `/`, or empty) to put before the image file names in the list
/// `atlist` so they point into `dir`: relative to the list's directory when `dir` is inside it,
/// absolute otherwise.
fn relative_dir(atlist: &Path, dir: &Path) -> String {
    let absolute = |p: &Path| -> PathBuf {
        let p = if p.as_os_str().is_empty() {
            Path::new(".")
        } else {
            p
        };
        std::path::absolute(p).unwrap_or_else(|_| p.into())
    };
    let list_dir = absolute(atlist.parent().unwrap_or(Path::new("")));
    let dir = absolute(dir);
    let relative = dir.strip_prefix(&list_dir).unwrap_or(&dir);
    let mut prefix = relative.to_string_lossy().replace('\\', "/");
    if !prefix.is_empty() && !prefix.ends_with('/') {
        prefix.push('/');
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_inside_the_list_directory_are_relative() {
        let prefix = relative_dir(Path::new("atlases/font.atlist"), Path::new("atlases/font"));
        assert_eq!(prefix, "font/");
        let prefix = relative_dir(Path::new("font.atlist"), Path::new("a/b"));
        assert_eq!(prefix, "a/b/");
        let prefix = relative_dir(Path::new("a/font.atlist"), Path::new("a"));
        assert_eq!(prefix, "");
    }

    #[test]
    fn images_outside_the_list_directory_are_absolute() {
        let prefix = relative_dir(Path::new("a/font.atlist"), Path::new("b"));
        assert!(Path::new(&prefix).is_absolute(), "{prefix}");
        assert!(prefix.ends_with("/b/"), "{prefix}");
    }

    #[test]
    fn default_chars_include_ascii_and_extras() {
        let chars = chars_to_render(&default_chars());
        for c in [' ', '~', 'ł', '’', '\u{00A0}'] {
            assert!(chars.contains(&c), "{c:?}");
        }
        assert!(chars.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn font_specs() {
        let spec = FontSpec::parse("# comment\nfont = Segoe UI Bold\n\nheight= 48\n").unwrap();
        assert_eq!(spec.font, "Segoe UI Bold");
        assert_eq!(spec.height, 48);
        assert_eq!(spec.chars, chars_to_render(&default_chars()));

        let spec = FontSpec::parse("font=a\nheight=8\nchars=cb\nadd-chars=ab").unwrap();
        assert_eq!(spec.chars, vec!['a', 'b', 'c']);

        for bad in [
            "height = 8",
            "font = a",
            "font = a\nheight = 0",
            "font = a\nheight = 8\nsize = 3",
            "font = a\nheight = 8\nno value",
            "font = a\nheight = 8\nchars =",
        ] {
            assert!(FontSpec::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn creates_images_and_their_list() {
        // Only on machines with Arial.
        if fonts::find_font("arial").is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("font-creator-test-{}", std::process::id()));
        let images = dir.join("font");
        let atlist = dir.join("font.atlist");
        let result = create_font_images("arial", 16, &['A', 'b', '\u{E000}'], &images, &atlist);
        let list = fs::read_to_string(&atlist);
        let a_exists = images.join("65.png").is_file();
        fs::remove_dir_all(&dir).unwrap();

        let result = result.unwrap();
        assert_eq!((result.written, result.skipped), (2, vec!['\u{E000}']));
        assert_eq!(list.unwrap(), "font/65.png\nfont/98.png\n");
        assert!(a_exists);
    }
}
