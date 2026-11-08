//! Compiles GLSL shaders to SPIR-V at build time.
//!
//! Every shader under `shaders/` (`.vert`, `.frag`, `.comp`, `.geom`, `.tesc`, `.tese`) is
//! compiled with `glslc` into `$OUT_DIR/shaders/<same relative path>.spv`, e.g.
//! `shaders/shader.vert` -> `$OUT_DIR/shaders/shader.vert.spv`.
//!
//! Embed a compiled shader with:
//! `include_bytes!(concat!(env!("OUT_DIR"), "/shaders/shader.vert.spv"))`
//!
//! `glslc` is looked up in this order:
//! 1. the `GLSLC` environment variable (full path to the executable),
//! 2. `$VULKAN_SDK/Bin/glslc` (`bin` on Linux/macOS),
//! 3. `glslc` on `PATH`.
//!
//! Everything under `resources/` is copied to an `assets/` directory next to the built
//! executable (e.g. `target/debug/assets/`), where `AssetsProvider` reads it at runtime.
//! Compiled shaders are copied there too, directly into `assets/`, e.g. `assets/shader.vert.spv`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SHADER_DIR: &str = "shaders";
const SHADER_EXTENSIONS: &[&str] = &["vert", "frag", "comp", "geom", "tesc", "tese"];
const ASSETS_SOURCE_DIR: &str = "resources";
const ASSETS_TARGET_DIR: &str = "assets";

fn main() {
    println!("cargo:rerun-if-changed={SHADER_DIR}");
    println!("cargo:rerun-if-changed={ASSETS_SOURCE_DIR}");
    println!("cargo:rerun-if-env-changed=GLSLC");
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let glslc = find_glslc();
    let optimize = env::var("PROFILE").map(|p| p == "release").unwrap_or(false);

    let assets_dir = assets_dir(&out_dir);
    copy_dir(Path::new(ASSETS_SOURCE_DIR), &assets_dir);

    let mut shaders = Vec::new();
    collect_shaders(Path::new(SHADER_DIR), &mut shaders);

    for source in shaders {
        let relative = source.strip_prefix(SHADER_DIR).unwrap();
        let mut output = out_dir.join(SHADER_DIR).join(relative).into_os_string();
        output.push(".spv");
        let output = PathBuf::from(output);

        fs::create_dir_all(output.parent().unwrap()).expect("failed to create shader output dir");

        let mut cmd = Command::new(&glslc);
        cmd.arg(&source).arg("-o").arg(&output);
        if optimize {
            cmd.arg("-O");
        }

        let result = cmd.output().unwrap_or_else(|e| {
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

        let asset = assets_dir.join(output.strip_prefix(out_dir.join(SHADER_DIR)).unwrap());
        fs::create_dir_all(asset.parent().unwrap()).expect("failed to create shader asset dir");
        fs::copy(&output, &asset).unwrap_or_else(|e| {
            panic!("failed to copy {} to {}: {e}", output.display(), asset.display())
        });
    }
}

/// The `assets/` directory next to the executable. `OUT_DIR` is
/// `<target>/<profile>/build/<package>-<hash>/out`, so the executable's directory is three
/// levels up.
fn assets_dir(out_dir: &Path) -> PathBuf {
    let exe_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("unexpected OUT_DIR layout");
    exe_dir.join(ASSETS_TARGET_DIR)
}

fn copy_dir(source: &Path, target: &Path) {
    fs::create_dir_all(target)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", target.display()));

    let entries = fs::read_dir(source)
        .unwrap_or_else(|e| panic!("failed to read asset dir {}: {e}", source.display()));

    for entry in entries {
        let path = entry.expect("failed to read dir entry").path();
        let destination = target.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &destination);
        } else {
            fs::copy(&path, &destination).unwrap_or_else(|e| {
                panic!("failed to copy {} to {}: {e}", path.display(), destination.display())
            });
        }
    }
}

fn find_glslc() -> PathBuf {
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
