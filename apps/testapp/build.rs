//! Builds the test app's assets.
//!
//! - Everything under `resources/` is copied to an `assets/` directory next to the built
//!   executable (e.g. `target/debug/assets/`), where `AssetsProvider` reads it at runtime.
//! - Every shader under `shaders/` is compiled with `glslc` into
//!   `$OUT_DIR/shaders/<same relative path>.spv` and copied to `assets/` too, e.g.
//!   `shaders/demo_000_viking_room_shader.vert` -> `assets/demo_000_viking_room_shader.vert.spv`.
//! - Every `atlases/<name>.atlist` is packed by the atlas creator into
//!   `$OUT_DIR/atlases/<name>.png` and `<name>.atlas`, copied to `assets/` too. The generated
//!   atlases are not in the repository; only the images and the `.atlist` lists are.
//!
//! See `feather-build` for how `glslc` is found.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let assets_dir = feather_build::assets_dir(&out_dir);

    feather_build::copy_dir(Path::new("resources"), &assets_dir);

    let shaders = feather_build::compile_shaders(Path::new("shaders"), &out_dir.join("shaders"));
    for shader in shaders {
        feather_build::copy_file(&shader.output, &assets_dir.join(&shader.relative));
    }

    let atlases = feather_build::build_atlases(Path::new("atlases"), &out_dir.join("atlases"));
    for atlas in atlases {
        for file in [&atlas.png, &atlas.metadata] {
            feather_build::copy_file(file, &assets_dir.join(file.file_name().unwrap()));
        }
    }
}
