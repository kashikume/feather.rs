//! Builds the test app's assets.
//!
//! - Everything under `resources/` but the OBJ files is copied to an `assets/` directory next
//!   to the built executable (e.g. `target/debug/assets/`), where `AssetsProvider` reads it at
//!   runtime.
//! - Every `resources/<name>.obj` is converted by the mesh converter into
//!   `$OUT_DIR/meshes/<name>.fmesh`, copied to `assets/` too; the scene files load those. Only
//!   the OBJ files are in the repository. (Deflated, except for Android: see
//!   `feather_build::build_meshes`.)
//! - Every shader under `shaders/` is compiled with `glslc` into
//!   `$OUT_DIR/shaders/<same relative path>.spv` and copied to `assets/` too, e.g.
//!   `shaders/demo_000_viking_room_shader.vert` -> `assets/demo_000_viking_room_shader.vert.spv`.
//! - Every `atlases/<name>.atlist` is packed by the atlas creator into
//!   `$OUT_DIR/atlases/<name>.png` and `<name>.atlas`, copied to `assets/` too. The generated
//!   atlases are not in the repository; only the images and the `.atlist` lists are.
//! - Every `atlases/<name>.font` is rendered by the font creator into character images in
//!   `$OUT_DIR/atlases/<name>/`, packed the same way. Only the `.font` files are in the
//!   repository.
//!
//! See `feather-build` for how `glslc` is found.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let assets_dir = feather_build::assets_dir(&out_dir);

    feather_build::copy_dir_filtered(Path::new("resources"), &assets_dir, |path| {
        !feather_build::is_obj(path)
    });
    for mesh in feather_build::build_meshes(Path::new("resources"), &out_dir.join("meshes")) {
        feather_build::copy_file(
            &mesh.output,
            &assets_dir.join(mesh.output.file_name().unwrap()),
        );
    }
    // OBJs copied by older builds must not end up in the app (the APK takes every file there).
    for entry in std::fs::read_dir(&assets_dir).expect("failed to read the assets dir") {
        let path = entry.expect("failed to read dir entry").path();
        if path.is_file() && feather_build::is_obj(&path) {
            std::fs::remove_file(&path)
                .unwrap_or_else(|e| panic!("failed to remove {}: {e}", path.display()));
        }
    }

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
