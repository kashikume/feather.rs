//! Compiles the shaders the unit tests use, from `testdata/shaders/` into `$OUT_DIR/shaders/`,
//! e.g. `testdata/shaders/basic.vert` -> `$OUT_DIR/shaders/basic.vert.spv`.
//!
//! Tests embed them with
//! `include_bytes!(concat!(env!("OUT_DIR"), "/shaders/basic.vert.spv"))`.
//! See `feather-build` for how `glslc` is found.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    feather_build::compile_shaders(Path::new("testdata/shaders"), &out_dir.join("shaders"));
}
