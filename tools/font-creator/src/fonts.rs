//! Finding the fonts installed in Windows.

use std::fs;
use std::path::{Path, PathBuf};

pub struct FontFile {
    /// The font's full name, e.g. "Arial Bold".
    pub name: String,
    pub path: PathBuf,
    /// The face index in a font collection (`.ttc`), 0 otherwise.
    pub index: u32,
}

/// The directories holding the fonts installed for all users and for the current user.
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
    dirs.push(Path::new(&windir).join("Fonts"));
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(Path::new(&local).join("Microsoft\\Windows\\Fonts"));
    }
    dirs
}

fn is_font_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    ["ttf", "otf", "ttc", "otc"]
        .iter()
        .any(|e| ext.eq_ignore_ascii_case(e))
}

fn font_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = font_dirs()
        .iter()
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flatten()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| is_font_file(path))
        .collect();
    files.sort();
    files
}

/// The faces in the font file at `path`, with their full names.
fn faces(path: &Path) -> Vec<FontFile> {
    let Ok(data) = fs::read(path) else {
        return Vec::new();
    };
    let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
    (0..count)
        .filter_map(|index| {
            let face = ttf_parser::Face::parse(&data, index).ok()?;
            Some(FontFile {
                name: full_name(&face)
                    .unwrap_or_else(|| path.file_stem().unwrap().to_string_lossy().into_owned()),
                path: path.to_path_buf(),
                index,
            })
        })
        .collect()
}

fn full_name(face: &ttf_parser::Face) -> Option<String> {
    let names = face.names();
    let find = |id| {
        names
            .into_iter()
            .filter(|n| n.name_id == id && n.is_unicode())
            // Prefer the English (US) name, as fonts may also carry localized ones.
            .min_by_key(|n| n.language_id != 0x0409)
            .and_then(|n| n.to_string())
    };
    find(ttf_parser::name_id::FULL_NAME).or_else(|| find(ttf_parser::name_id::FAMILY))
}

/// Every face of every installed font, sorted by name.
pub fn installed_fonts() -> Vec<FontFile> {
    let mut fonts: Vec<FontFile> = font_files().iter().flat_map(|p| faces(p)).collect();
    fonts.sort_by_key(|f| f.name.to_lowercase());
    fonts
}

/// The font `name`: a path to a font file, a file name in a Windows font directory (with or
/// without the extension, e.g. "arialbd") or a font's full name (e.g. "Arial Bold"), compared
/// without regard to case.
pub fn find_font(name: &str) -> Option<FontFile> {
    let path = Path::new(name);
    if path.is_file() {
        return faces(path).into_iter().next();
    }

    let files = font_files();
    let wanted = name.to_lowercase();
    let file_matches = |p: &PathBuf| {
        [p.file_name(), p.file_stem()]
            .into_iter()
            .flatten()
            .any(|n| n.to_string_lossy().to_lowercase() == wanted)
    };
    if let Some(file) = files.iter().find(|p| file_matches(p)) {
        return faces(file).into_iter().next();
    }
    files
        .iter()
        .flat_map(|p| faces(p))
        .find(|f| f.name.to_lowercase() == wanted)
}
