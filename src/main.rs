use anyhow::Result;
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

mod feather;
use feather::app::App;

mod testapp;
use testapp::testapp::TestApp;

#[rustfmt::skip]
fn main() -> Result<()> {
    pretty_env_logger::init();

    // Window

    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Feather development app")
        .with_inner_size(LogicalSize::new(1024, 768))
        .build(&event_loop)?;

    // App

    let mut app = unsafe { App::create(&window, Box::new(TestApp::new()))? };
    app.run(&window, event_loop)?;

    Ok(())
}
