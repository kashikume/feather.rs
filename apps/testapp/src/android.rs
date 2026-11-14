//! The test app's entry point on Android.

use anyhow::Result;
use log::LevelFilter;
use winit::event_loop::EventLoop;
use winit::platform::android::EventLoopBuilderExtAndroid;
use winit::window::Window;

use feather::androidassetsprovider::AndroidAssetsProvider;
use feather::app::App;
use feather::AndroidApp;

use crate::testapp::testapp::{demo_window_title, TestApp};

/// Called by `android-activity` on its own thread once the activity is created; the C++
/// engine's `android_main` in GuitarPrompter. Runs the app until the activity is finished
/// (back), then ends the process.
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(LevelFilter::Info)
            .with_tag("feather"),
    );
    // Panics would otherwise only reach logcat as a bare abort.
    std::panic::set_hook(Box::new(|info| log::error!("{info}")));

    log::info!("Starting the feather test app for Android.");
    if let Err(e) = run(app) {
        log::error!("{e:#}");
    }
    log::info!("The feather test app has ended.");
    // Android may call `android_main` again in this process for a new activity, and winit
    // can't make a second event loop in a process: end the process instead. With `_exit`, not
    // `std::process::exit`: the latter runs C++ static destructors, and GameActivity's
    // (`GameTextInput`) aborts the process on a stale JNI reference. The engine and the app are
    // already destroyed; nothing else needs to run.
    unsafe { _exit(0) }
}

extern "C" {
    /// bionic's `_exit`: ends the process without running `atexit` handlers or destructors.
    fn _exit(status: i32) -> !;
}

fn run(app: AndroidApp) -> Result<()> {
    let event_loop = EventLoop::builder().with_android_app(app.clone()).build()?;
    let window = Window::default_attributes().with_title(demo_window_title(0));
    let assets = Box::new(AndroidAssetsProvider::new(&app));
    App::run(
        event_loop,
        window,
        Box::new(TestApp::with_assets(assets, 0)?),
    )
}

/// `MainActivity.nativeRequestExit()`: back was pressed and the activity is finishing. Ends the
/// event loop (winit 0.30 doesn't, on Android), so the engine and the app are destroyed and
/// `android_main` returns.
#[no_mangle]
extern "system" fn Java_com_kashikume_feather_testapp_MainActivity_nativeRequestExit(
    _env: *mut std::ffi::c_void,
    _this: *mut std::ffi::c_void,
) {
    feather::app::request_exit();
}
