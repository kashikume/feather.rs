//! Helpers for the build scripts (`build.rs`) of the feather crates and apps.
//!
//! - [`compile_shaders`] compiles GLSL shaders to SPIR-V with `glslc`.
//! - [`copy_dir`], [`copy_file`] and [`assets_dir`] put an app's assets next to its executable.
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
    println!("cargo:rerun-if-changed={}", source.display());
    copy_dir_inner(source, target);
}

fn copy_dir_inner(source: &Path, target: &Path) {
    fs::create_dir_all(target)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", target.display()));

    let entries = fs::read_dir(source)
        .unwrap_or_else(|e| panic!("failed to read dir {}: {e}", source.display()));

    for entry in entries {
        let path = entry.expect("failed to read dir entry").path();
        let destination = target.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir_inner(&path, &destination);
        } else {
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
