use anyhow::{bail, Result};
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

use feather::app::App;

mod testapp;
use testapp::testapp::{demo_window_title, TestApp, DEMOS};

/// Usage: `testapp [demo]`, where `demo` is the number of the demo to show first
/// (default 0). The left and right arrow keys, or the buttons on the window's sides, switch demos.
#[rustfmt::skip]
fn main() -> Result<()> {
    pretty_env_logger::init();

    let demo = demo_from_args(std::env::args().skip(1))?;

    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title(demo_window_title(demo))
        .with_inner_size(LogicalSize::new(1024, 768))
        .build(&event_loop)?;

    let mut app = unsafe { App::create(&window, Box::new(TestApp::new(demo)?))? };
    app.run(&window, event_loop)?;

    Ok(())
}

/// The demo number from the command line arguments (without the program name).
fn demo_from_args(mut args: impl Iterator<Item = String>) -> Result<usize> {
    let demo = match args.next() {
        None => 0,
        Some(arg) => match arg.parse::<usize>() {
            Ok(demo) if demo < DEMOS.len() => demo,
            _ => bail!("Unknown demo `{arg}`.
{}", usage()),
        },
    };
    if let Some(extra) = args.next() {
        bail!("Unexpected argument `{extra}`.
{}", usage());
    }
    Ok(demo)
}

fn usage() -> String {
    let demos: Vec<String> = DEMOS
        .iter()
        .enumerate()
        .map(|(i, d)| format!("  {i}  {}", d.name))
        .collect();
    format!("Usage: testapp [demo]
Demos:
{}", demos.join("
"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<usize> {
        demo_from_args(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn demo_defaults_to_zero() {
        assert_eq!(parse(&[]).unwrap(), 0);
    }

    #[test]
    fn demo_number_is_read() {
        assert_eq!(parse(&["1"]).unwrap(), 1);
    }

    #[test]
    fn bad_arguments_fail_with_usage() {
        for args in [&["99"][..], &["x"], &["-1"], &["0", "1"]] {
            let err = parse(args).unwrap_err().to_string();
            assert!(err.contains("Usage: testapp [demo]"), "{err}");
            assert!(err.contains("0  Viking room"), "{err}");
        }
    }
}
