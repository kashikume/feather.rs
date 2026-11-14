//! The window's Vulkan surface, on each platform.
//!
//! `vulkanalia::window` handles Windows, Linux and macOS, not Android: on Android the surface is
//! made here from the window's `ANativeWindow`, with `VK_KHR_android_surface`.

use anyhow::Result;
use vulkanalia::prelude::v1_0::*;
use winit::window::Window;

/// The instance extensions needed to make a surface for `window`.
pub fn required_instance_extensions(window: &Window) -> Vec<&'static vk::ExtensionName> {
    #[cfg(target_os = "android")]
    {
        let _ = window;
        vec![
            &vk::KHR_SURFACE_EXTENSION.name,
            &vk::KHR_ANDROID_SURFACE_EXTENSION.name,
        ]
    }
    #[cfg(not(target_os = "android"))]
    {
        vulkanalia::window::get_required_instance_extensions(window).to_vec()
    }
}

/// Creates the Vulkan surface of `window`.
///
/// On Android the window has a native window only between `Resumed` and `Suspended`; this
/// fails outside of that.
pub unsafe fn create_surface(instance: &Instance, window: &Window) -> Result<vk::SurfaceKHR> {
    #[cfg(target_os = "android")]
    {
        use anyhow::{anyhow, Context};
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use vk::KhrAndroidSurfaceExtension;

        let handle = window
            .window_handle()
            .context("the window has no native window (not resumed yet?)")?;
        let RawWindowHandle::AndroidNdk(handle) = handle.as_raw() else {
            return Err(anyhow!("not an Android window: {:?}", handle.as_raw()));
        };
        let info = vk::AndroidSurfaceCreateInfoKHR::builder()
            .window(handle.a_native_window.as_ptr().cast());
        Ok(instance.create_android_surface_khr(&info, None)?)
    }
    #[cfg(not(target_os = "android"))]
    {
        Ok(vulkanalia::window::create_surface(
            instance, window, window,
        )?)
    }
}
