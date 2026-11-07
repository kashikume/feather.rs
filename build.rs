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

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SHADER_DIR: &str = "shaders";
const SHADER_EXTENSIONS: &[&str] = &["vert", "frag", "comp", "geom", "tesc", "tese"];

fn main() {
    println!("cargo:rerun-if-changed={SHADER_DIR}");
    println!("cargo:rerun-if-env-changed=GLSLC");
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let glslc = find_glslc();
    let optimize = env::var("PROFILE").map(|p| p == "release").unwrap_or(false);

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
