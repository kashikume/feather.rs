//! The test app's entry point on Android.

use anyhow::Result;
use log::LevelFilter;
use winit::event_loop::EventLoopBuilder;
use winit::platform::android::EventLoopBuilderExtAndroid;
use winit::window::WindowBuilder;

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
    // can't make a second event loop in a process: end the process instead.
    std::process::exit(0);
}

fn run(app: AndroidApp) -> Result<()> {
    let event_loop = EventLoopBuilder::new().with_android_app(app.clone()).build()?;
    let window = WindowBuilder::new()
        .with_title(demo_window_title(0))
        .build(&event_loop)?;
    let assets = Box::new(AndroidAssetsProvider::new(&app));
    App::run(window, event_loop, Box::new(TestApp::with_assets(assets, 0)?))
}
