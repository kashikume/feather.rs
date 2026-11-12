//! Helpers for the build scripts (`build.rs`) of the feather crates and apps.
//!
//! - [`compile_shaders`] compiles GLSL shaders to SPIR-V with `glslc`.
//! - [`build_atlases`] packs the images listed in `.atlist` files into atlases, and renders
//!   the fonts described in `.font` files into atlases of their characters.
//! - [`build_meshes`] converts Wavefront OBJ files into `.fmesh` files with the mesh converter.
//! - [`copy_dir`], [`copy_dir_filtered`], [`copy_file`] and [`assets_dir`] put an app's assets
//!   next to its executable.
//!
//! `glslc` is looked up in this order (see [`find_glslc`]):
//! 1. the `GLSLC` environment variable (full path to the executable),
//! 2. `$VULKAN_SDK/Bin/glslc` (`bin` on Linux/macOS),
//! 3. `glslc` on `PATH`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// File extensions of the shaders [`compile_shaders`] picks up.
pub const SHADER_EXTENSIONS: &[&str] = &["vert", "frag", "comp", "geom", "tesc", "tese"];

/// A shader compiled by [`compile_shaders`].
pub struct CompiledShader {
    /// The GLSL source, e.g. `shaders/sub/basic.vert`.
    pub source: PathBuf,
    /// The SPIR-V path relative to the output directory, e.g. `sub/basic.vert.spv`.
    pub relative: PathBuf,
    /// The SPIR-V file, e.g. `$OUT_DIR/shaders/sub/basic.vert.spv`.
    pub output: PathBuf,
}

/// Compiles every shader under `shader_dir` with `glslc` into `out_dir`, keeping the relative
/// path and appending `.spv`: `shader_dir/sub/basic.vert` -> `out_dir/sub/basic.vert.spv`.
///
/// Also prints the `cargo:rerun-if-*` lines for the shaders and the `glslc` lookup.
/// Panics (failing the build) if `glslc` can't be run or a shader doesn't compile.
pub fn compile_shaders(shader_dir: &Path, out_dir: &Path) -> Vec<CompiledShader> {
    println!("cargo:rerun-if-changed={}", shader_dir.display());
    println!("cargo:rerun-if-env-changed=GLSLC");
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");

    let glslc = find_glslc();

    let mut sources = Vec::new();
    collect_shaders(shader_dir, &mut sources);

    sources
        .into_iter()
        .map(|source| {
            let mut relative = source.strip_prefix(shader_dir).unwrap().as_os_str().to_owned();
            relative.push(".spv");
            let relative = PathBuf::from(relative);
            let output = out_dir.join(&relative);

            fs::create_dir_all(output.parent().unwrap())
                .expect("failed to create shader output dir");

            // No `-O`, also in release builds: it strips the debug names that shader
            // reflection (`ShaderLayout`) uses as uniform names, and may drop unused uniforms.
            // Drivers optimize SPIR-V themselves.
            let result = Command::new(&glslc)
                .arg(&source)
                .arg("-o")
                .arg(&output)
                .output()
                .unwrap_or_else(|e| {
                    panic!(
                        "failed to run glslc ({}): {e}\n\
                         Install the Vulkan SDK (https://vulkan.lunarg.com) or set GLSLC to the glslc path.",
                        glslc.display()
                    )
                });

            if !result.status.success() {
                panic!(
                    "glslc failed to compile {}:\n{}",
                    source.display(),
                    String::from_utf8_lossy(&result.stderr)
                );
            }

            CompiledShader {
                source,
                relative,
                output,
            }
        })
        .collect()
}

/// The directory of the executable being built, given the build script's `OUT_DIR`
/// (`<target>/<profile>/build/<package>-<hash>/out`, so three levels up).
pub fn exe_dir(out_dir: &Path) -> PathBuf {
    out_dir
        .ancestors()
        .nth(3)
        .expect("unexpected OUT_DIR layout")
        .to_path_buf()
}

/// The `assets/` directory next to the executable, where `WindowsAssetsProvider` reads assets.
pub fn assets_dir(out_dir: &Path) -> PathBuf {
    exe_dir(out_dir).join("assets")
}

/// Copies the directory `source` into `target` recursively, creating `target` if needed.
/// Prints `cargo:rerun-if-changed` for `source`.
pub fn copy_dir(source: &Path, target: &Path) {
    copy_dir_filtered(source, target, |_| true);
}

/// Like [`copy_dir`], but copies only the files for which `keep` is true (e.g. not the OBJ
/// files [`build_meshes`] converts).
pub fn copy_dir_filtered(source: &Path, target: &Path, keep: impl Fn(&Path) -> bool) {
    println!("cargo:rerun-if-changed={}", source.display());
    copy_dir_inner(source, target, &keep);
}

fn copy_dir_inner(source: &Path, target: &Path, keep: &dyn Fn(&Path) -> bool) {
    fs::create_dir_all(target)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", target.display()));

    let entries = fs::read_dir(source)
        .unwrap_or_else(|e| panic!("failed to read dir {}: {e}", source.display()));

    for entry in entries {
        let path = entry.expect("failed to read dir entry").path();
        let destination = target.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir_inner(&path, &destination, keep);
        } else if keep(&path) {
            copy_file(&path, &destination);
        }
    }
}

/// Copies one file, creating the target's directory if needed.
pub fn copy_file(source: &Path, target: &Path) {
    fs::create_dir_all(target.parent().unwrap())
        .unwrap_or_else(|e| panic!("failed to create dir for {}: {e}", target.display()));
    fs::copy(source, target).unwrap_or_else(|e| {
        panic!(
            "failed to copy {} to {}: {e}",
            source.display(),
            target.display()
        )
    });
}

/// A mesh converted by [`build_meshes`].
pub struct BuiltMesh {
    /// The OBJ file, e.g. `resources/car_mesh.obj`.
    pub source: PathBuf,
    /// The `.fmesh` file, e.g. `$OUT_DIR/meshes/car_mesh.fmesh`.
    pub output: PathBuf,
}

/// Whether `path` is a Wavefront OBJ file (`.obj`), which [`build_meshes`] converts.
pub fn is_obj(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("obj"))
}

/// Converts every OBJ file directly in `source_dir` into an `.fmesh` file in `out_dir` with
/// the mesh converter's default settings: `source_dir/<name>.obj` -> `out_dir/<name>.fmesh`.
///
/// The data is deflated (when that saves 10% or more), except when building for Android: the
/// APK deflates its assets already, so a second compression would gain next to nothing and
/// make the device inflate every mesh twice.
///
/// Prints `cargo:rerun-if-changed` for `source_dir`. Panics (failing the build) if an OBJ
/// can't be converted.
pub fn build_meshes(source_dir: &Path, out_dir: &Path) -> Vec<BuiltMesh> {
    println!("cargo:rerun-if-changed={}", source_dir.display());
    let android = env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "android");
    let settings = mesh_converter::Settings {
        compression: if android {
            mesh_converter::Compression::Off
        } else {
            mesh_converter::Compression::Auto
        },
        ..Default::default()
    };

    let mut sources: Vec<PathBuf> = fs::read_dir(source_dir)
        .unwrap_or_else(|e| panic!("failed to read dir {}: {e}", source_dir.display()))
        .map(|entry| entry.expect("failed to read dir entry").path())
        .filter(|path| path.is_file() && is_obj(path))
        .collect();
    sources.sort();

    sources
        .into_iter()
        .map(|source| {
            let name = Path::new(source.file_name().unwrap());
            let output = out_dir.join(name.with_extension("fmesh"));
            let (conversion, _) = mesh_converter::convert_file(&source, &output, &settings, false)
                .unwrap_or_else(|e| panic!("failed to convert {}: {e:#}", source.display()));
            for warning in conversion.meshes.iter().flat_map(|m| &m.warnings) {
                println!("cargo:warning={}: {warning}", source.display());
            }
            BuiltMesh { source, output }
        })
        .collect()
}

/// An atlas created by [`build_atlases`].
pub struct BuiltAtlas {
    /// The source: an input list, e.g. `atlases/sprites.atlist`, or a font description, e.g.
    /// `atlases/font.font`.
    pub source: PathBuf,
    /// The atlas image, e.g. `$OUT_DIR/atlases/sprites.png`.
    pub png: PathBuf,
    /// The metadata, e.g. `$OUT_DIR/atlases/sprites.atlas`.
    pub metadata: PathBuf,
    pub width: usize,
    pub height: usize,
}

/// Atlas sizes [`build_atlases`] tries, smallest first: powers of two from 64 to 8192, each
/// as a 2:1 rectangle and then as a square.
fn atlas_sizes() -> impl Iterator<Item = (usize, usize)> {
    (6..=13).flat_map(|p| [(1 << p, 1 << (p - 1)), (1 << p, 1 << p)])
}

/// Creates an atlas for every `.atlist` and `.font` file directly in `atlas_dir`:
/// `atlas_dir/<name>.atlist` (or `.font`) -> `out_dir/<name>.png` and `out_dir/<name>.atlas`.
/// Each atlas gets the smallest size from 64x32, 64x64, 128x64, ... 8192x8192 its images fit
/// in.
///
/// - An `.atlist` file lists PNG images, one per line, relative to the list; the atlas creator
///   packs them.
/// - A `.font` file names a font installed in Windows, the image height and the characters
///   (see `font_creator::FontSpec`). The font creator renders a PNG image per character into
///   `out_dir/<name>/` and lists them in `out_dir/<name>.atlist`, which is then packed. The
///   images are named by code point (`65` for `A`), and so are the textures of the atlas.
///
/// Prints `cargo:rerun-if-changed` for `atlas_dir` (Cargo then watches every file in it).
/// Panics (failing the build) if an atlas can't be created, or if two sources have the same
/// name.
pub fn build_atlases(atlas_dir: &Path, out_dir: &Path) -> Vec<BuiltAtlas> {
    println!("cargo:rerun-if-changed={}", atlas_dir.display());
    fs::create_dir_all(out_dir)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", out_dir.display()));

    let mut sources: Vec<PathBuf> = fs::read_dir(atlas_dir)
        .unwrap_or_else(|e| panic!("failed to read dir {}: {e}", atlas_dir.display()))
        .map(|entry| entry.expect("failed to read dir entry").path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|e| e == "atlist" || e == "font")
        })
        .collect();
    sources.sort();
    for pair in sources.windows(2) {
        if pair[0].file_stem() == pair[1].file_stem() {
            panic!(
                "{} and {} would make the same atlas",
                pair[0].display(),
                pair[1].display()
            );
        }
    }

    sources
        .into_iter()
        .map(|source| {
            let stem = source.file_stem().unwrap();
            let list = if source.extension().is_some_and(|e| e == "font") {
                let list = out_dir.join(stem).with_extension("atlist");
                render_font(&source, &out_dir.join(stem), &list);
                list
            } else {
                source.clone()
            };
            pack_atlas(source, &list, out_dir)
        })
        .collect()
}

/// Renders the font described in the `.font` file `source` into images in `image_dir`, listed
/// in `list`. Panics if it can't.
fn render_font(source: &Path, image_dir: &Path, list: &Path) {
    let spec = font_creator::FontSpec::load(source).unwrap_or_else(|e| panic!("{e}"));
    // Images of characters no longer in the spec must not stay around.
    if image_dir.exists() {
        fs::remove_dir_all(image_dir)
            .unwrap_or_else(|e| panic!("failed to clear {}: {e}", image_dir.display()));
    }
    font_creator::create_font_images(&spec.font, spec.height, &spec.chars, image_dir, list)
        .unwrap_or_else(|e| panic!("failed to render font {}: {e}", source.display()));
}

/// Packs the images listed in `list` into `out_dir/<name of source>.png` and `.atlas`, in the
/// smallest atlas they fit in.
fn pack_atlas(source: PathBuf, list: &Path, out_dir: &Path) -> BuiltAtlas {
    let stem = source.file_stem().unwrap();
    let png = out_dir.join(stem).with_extension("png");
    let metadata = out_dir.join(stem).with_extension("atlas");
    for (width, height) in atlas_sizes() {
        match atlas_creator::create_atlas(list, &png, &metadata, width, height) {
            Ok(()) => {
                return BuiltAtlas {
                    source,
                    png,
                    metadata,
                    width,
                    height,
                }
            }
            Err(atlas_creator::AtlasError::DoesNotFit) => continue,
            Err(e) => panic!("failed to create atlas {}: {e}", source.display()),
        }
    }
    panic!(
        "failed to create atlas {}: the images don't fit in 8192x8192",
        source.display()
    )
}

/// Finds `glslc`: `$GLSLC`, then the Vulkan SDK, then `PATH`.
pub fn find_glslc() -> PathBuf {
    if let Some(path) = env::var_os("GLSLC") {
        return PathBuf::from(path);
    }

    if let Some(sdk) = env::var_os("VULKAN_SDK") {
        let exe = if cfg!(windows) { "glslc.exe" } else { "glslc" };
        for bin in ["Bin", "bin"] {
            let candidate = Path::new(&sdk).join(bin).join(exe);
            if candidate.is_file() {
                return candidate;
            }
        }
    }

    PathBuf::from("glslc")
}

fn collect_shaders(dir: &Path, shaders: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read shader dir {}: {e}", dir.display()));

    for entry in entries {
        let path = entry.expect("failed to read dir entry").path();
        if path.is_dir() {
            collect_shaders(&path, shaders);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| SHADER_EXTENSIONS.contains(&e))
        {
            shaders.push(path);
        }
    }
}
