//! The test app's demos, as a library: used by `main.rs` on Windows, and loaded as
//! `libfeather_testapp.so` by the Android activity, which calls `android_main`.

pub mod testapp;

#[cfg(target_os = "android")]
mod android;
