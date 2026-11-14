//! Converts Wavefront OBJ files into `.fmesh` files, feather's binary meshes (see
//! `doc/fmesh-format.md`).
//!
//! A port of the C++ feather's `ObjMeshConverter`, without its precision setting: every number
//! of the OBJ is read as text, and each stream (positions, normals, texture coordinates) is
//! stored as integers with as many decimals as its most precise number, so the engine gets back
//! exactly the floats the OBJ's text stands for. Streams that can't be stored that way are kept
//! as `f32`.
//!
//! The mesh is the one `MeshBuilderObjFile` makes from the OBJ: faces triangulated as fans,
//! identical vertices merged (in the order of first use), V flipped to `1 - v`. Normals come
//! from the OBJ, or are computed (flat or soft).
//!
//! Used by the `mesh-converter` executable and, through [`convert_file`], by build scripts.

mod convert;
mod decimal;
pub mod normals;
mod obj;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
pub use feather_fmesh::{Compression, EncodingChoice};
use feather_fmesh::{MeshData, Written};
pub use normals::Normals;

/// How to convert. The default is what the build uses on the desktop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// One mesh per `o` (or, without any, per `g`) object, instead of all in one.
    pub split: bool,
    pub encoding: EncodingChoice,
    pub compression: Compression,
    /// The most decimals a stream may have to be stored as integers.
    pub max_decimals: u32,
    pub normals: Normals,
    /// Store `1 - v` (the default; what `MeshBuilderObjFile` does).
    pub flip_v: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            split: false,
            encoding: EncodingChoice::Auto,
            compression: Compression::Auto,
            max_decimals: 9,
            normals: Normals::FromObj,
            flip_v: true,
        }
    }
}

/// Counts about a converted mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub triangles: usize,
    /// Triangle corners: three per triangle.
    pub corners: usize,
    /// Vertices once identical corners are merged.
    pub vertices: usize,
}

/// One converted mesh.
pub struct Converted {
    /// The object's name with [`Settings::split`], `None` otherwise.
    pub object: Option<String>,
    pub mesh: MeshData,
    /// The `.fmesh` file.
    pub written: Written,
    pub stats: Stats,
    /// Things the user should know: streams stored as `f32`, normals replaced...
    pub warnings: Vec<String>,
    /// Details for `--verbose`: polygons split.
    pub notes: Vec<String>,
    /// The values each triangle corner must have when read back (position, normal, UV).
    expected: Vec<[f32; 8]>,
    /// The difference allowed on normals (computed normals are rounded to 4 decimals).
    normals_tolerance: f32,
}

/// What [`convert`] makes of an OBJ file.
pub struct Conversion {
    pub meshes: Vec<Converted>,
    /// Lines of the OBJ not used (`l`, `p`, curves...), with their number.
    pub skipped: Vec<(usize, String)>,
}

/// Converts the text of an OBJ file into one mesh, or one per object with
/// [`Settings::split`].
pub fn convert(obj_text: &str, settings: &Settings) -> Result<Conversion> {
    let obj = obj::Obj::parse(obj_text)?;
    if obj.faces.is_empty() {
        bail!("the OBJ has no faces");
    }

    let meshes = if settings.split {
        // Objects by `o`, or by `g` when there's no `o`; faces of objects with the same name
        // go together.
        let (names, of_face): (&[String], Box<dyn Fn(&obj::Face) -> Option<u32>>) =
            if !obj.objects.is_empty() {
                (&obj.objects, Box::new(|f| f.object))
            } else {
                (&obj.groups, Box::new(|f| f.group))
            };
        let mut objects: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, face) in obj.faces.iter().enumerate() {
            let name = of_face(face)
                .map(|o| names[o as usize].as_str())
                .filter(|name| !name.is_empty())
                .unwrap_or("unnamed");
            match objects.iter_mut().find(|(n, _)| n == name) {
                Some((_, faces)) => faces.push(i),
                None => objects.push((name.to_string(), vec![i])),
            }
        }
        objects
            .into_iter()
            .map(|(name, faces)| {
                convert::convert_faces(&obj, &faces, settings, Some(name.clone()))
                    .with_context(|| format!("object {name}"))
            })
            .collect::<Result<_>>()?
    } else {
        let faces: Vec<usize> = (0..obj.faces.len()).collect();
        vec![convert::convert_faces(&obj, &faces, settings, None)?]
    };

    Ok(Conversion {
        meshes,
        skipped: obj.skipped,
    })
}

/// Reads `bytes` as an `.fmesh` file and checks that every triangle corner has the values the
/// OBJ gave it.
pub fn check(converted: &Converted, bytes: &[u8]) -> Result<()> {
    let mesh = feather_fmesh::read(bytes).map_err(|e| anyhow!("{e}"))?;
    if mesh.indices.len() != converted.expected.len() {
        bail!(
            "{} triangle corners read back, {} expected",
            mesh.indices.len(),
            converted.expected.len()
        );
    }
    for (k, (&index, expected)) in mesh.indices.iter().zip(&converted.expected).enumerate() {
        let i = index as usize;
        let read = [
            mesh.positions[3 * i],
            mesh.positions[3 * i + 1],
            mesh.positions[3 * i + 2],
            mesh.normals[3 * i],
            mesh.normals[3 * i + 1],
            mesh.normals[3 * i + 2],
            mesh.uvs[2 * i],
            mesh.uvs[2 * i + 1],
        ];
        for c in 0..8 {
            let tolerance = if (3..6).contains(&c) {
                converted.normals_tolerance
            } else {
                0.0
            };
            // NaN never compares equal; it's still the right value if both are NaN.
            let same = (read[c] - expected[c]).abs() <= tolerance
                || (read[c].is_nan() && expected[c].is_nan());
            if !same {
                let what = [
                    "position x",
                    "position y",
                    "position z",
                    "normal x",
                    "normal y",
                    "normal z",
                    "u",
                    "v",
                ][c];
                bail!(
                    "triangle {}, corner {}: {what} is {} instead of {}",
                    k / 3,
                    k % 3,
                    read[c],
                    expected[c]
                );
            }
        }
    }
    Ok(())
}

/// The file name for an object of a split OBJ: `<output's stem>_<object>.fmesh`, the object's
/// name with anything but letters, digits, `_`, `-` and `.` turned into `_`.
pub fn split_path(output: &Path, object: &str) -> PathBuf {
    let object: String = object
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_-.".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    output.with_file_name(format!("{stem}_{object}.fmesh"))
}

/// Converts the OBJ file `input` into `output` (or, with [`Settings::split`], into files named
/// by [`split_path`]), creating the output's directory if needed. With `check`, reads every
/// file back and [`check`]s it.
pub fn convert_file(
    input: &Path,
    output: &Path,
    settings: &Settings,
    check_output: bool,
) -> Result<(Conversion, Vec<PathBuf>)> {
    let text =
        fs::read_to_string(input).with_context(|| format!("can't read {}", input.display()))?;
    let conversion = convert(&text, settings)?;
    if let Some(dir) = output.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).with_context(|| format!("can't create {}", dir.display()))?;
    }
    let mut paths = Vec::new();
    for converted in &conversion.meshes {
        let path = match &converted.object {
            Some(object) => split_path(output, object),
            None => output.to_path_buf(),
        };
        fs::write(&path, &converted.written.bytes)
            .with_context(|| format!("can't write {}", path.display()))?;
        if check_output {
            let bytes =
                fs::read(&path).with_context(|| format!("can't read back {}", path.display()))?;
            check(converted, &bytes)
                .with_context(|| format!("check of {} failed", path.display()))?;
        }
        paths.push(path);
    }
    Ok((conversion, paths))
}

#[cfg(test)]
mod tests;
