use anyhow::Result;
use log::info;
use vulkanalia::prelude::v1_0::*;
use winit::window::Window;

use vulkanalia::vk::KhrSwapchainExtension;

use super::appdata::AppData;
use super::images::create_image_view;
use super::queuefamilyindices::QueueFamilyIndices;
use super::swapchainsupport::SwapchainSupport;

#[derive(Clone, Debug, Default)]
pub struct Swapchain {
    pub swapchain_format: vk::Format,
    pub swapchain_extent: vk::Extent2D,
    pub swapchain: vk::SwapchainKHR,
    pub swapchain_images: Vec<vk::Image>,
    pub swapchain_image_views: Vec<vk::ImageView>,
    /// Whether presenting is expected to report `SUBOPTIMAL_KHR`: the image is drawn unrotated
    /// on a rotated display (see `choose_pre_transform`), and the display system rotates it. That
    /// is a choice, so recreating the swapchain wouldn't help.
    pub suboptimal_expected: bool,
}

/// The surface format to draw in: 8-bit sRGB, so that the shaders' linear colors are encoded to
/// sRGB when written. Windows offers `B8G8R8A8_SRGB`; Android usually only `R8G8B8A8_SRGB`.
/// Without either, the first format (likely UNORM: the image looks too dark).
fn choose_surface_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
    [vk::Format::B8G8R8A8_SRGB, vk::Format::R8G8B8A8_SRGB]
        .iter()
        .find_map(|wanted| {
            formats.iter().cloned().find(|f| {
                f.format == *wanted && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
        })
        .unwrap_or_else(|| formats[0])
}

fn choose_present_mode(present_modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    present_modes
        .iter()
        .cloned()
        .find(|m| *m == vk::PresentModeKHR::MAILBOX)
        .unwrap_or(vk::PresentModeKHR::FIFO)
}

/// How the image is combined with what is behind the window: `OPAQUE` (alpha ignored) if the
/// surface supports it. Many Android drivers support only `INHERIT` (the window system's
/// setting, opaque for an activity).
fn choose_composite_alpha(supported: vk::CompositeAlphaFlagsKHR) -> vk::CompositeAlphaFlagsKHR {
    [
        vk::CompositeAlphaFlagsKHR::OPAQUE,
        vk::CompositeAlphaFlagsKHR::INHERIT,
        vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
    ]
    .into_iter()
    .find(|mode| supported.contains(*mode))
    .unwrap_or(vk::CompositeAlphaFlagsKHR::OPAQUE)
}

/// The transform the image is drawn with: none (`IDENTITY`) whenever the surface allows it, and
/// the display system rotates the image to the screen.
///
/// On a phone in landscape the display is rotated from its natural (portrait) orientation, and
/// `current_transform` is `ROTATE_90` or `ROTATE_270`. Passing that instead would mean the engine
/// draws rotated itself (pre-rotation; not done yet).
fn choose_pre_transform(capabilities: &vk::SurfaceCapabilitiesKHR) -> vk::SurfaceTransformFlagsKHR {
    if capabilities
        .supported_transforms
        .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
    {
        vk::SurfaceTransformFlagsKHR::IDENTITY
    } else {
        capabilities.current_transform
    }
}

/// The size of the swapchain's images.
///
/// When the surface states its size (`current_extent`) and the image is drawn with the surface's
/// own transform, that size. Otherwise the window's size (`window_size`), within the surface's
/// limits: when the display is rotated (Android in landscape) and the image is not pre-rotated,
/// the image has the window's orientation.
fn choose_extent(
    capabilities: &vk::SurfaceCapabilitiesKHR,
    pre_transform: vk::SurfaceTransformFlagsKHR,
    window_size: (u32, u32),
) -> vk::Extent2D {
    if capabilities.current_extent.width != u32::MAX
        && pre_transform == capabilities.current_transform
    {
        return capabilities.current_extent;
    }
    let (min, max) = (capabilities.min_image_extent, capabilities.max_image_extent);
    vk::Extent2D {
        width: window_size.0.clamp(min.width, max.width),
        height: window_size.1.clamp(min.height, max.height),
    }
}

/// Logs what the surface offers and what was chosen: on a new device, the first thing to look at
/// when the image is wrong.
fn log_choices(
    support: &SwapchainSupport,
    format: vk::SurfaceFormatKHR,
    present_mode: vk::PresentModeKHR,
    composite_alpha: vk::CompositeAlphaFlagsKHR,
    pre_transform: vk::SurfaceTransformFlagsKHR,
    extent: vk::Extent2D,
    window_size: (u32, u32),
) {
    let caps = &support.capabilities;
    info!(
        "Surface: current extent {}x{}, image extent {}x{} to {}x{}, window {}x{}, \
         current transform {:?}, supported transforms {:?}, composite alpha {:?}, \
         {} to {} images",
        caps.current_extent.width,
        caps.current_extent.height,
        caps.min_image_extent.width,
        caps.min_image_extent.height,
        caps.max_image_extent.width,
        caps.max_image_extent.height,
        window_size.0,
        window_size.1,
        caps.current_transform,
        caps.supported_transforms,
        caps.supported_composite_alpha,
        caps.min_image_count,
        caps.max_image_count,
    );
    info!(
        "Surface formats: {:?}; present modes: {:?}",
        support
            .formats
            .iter()
            .map(|f| (f.format, f.color_space))
            .collect::<Vec<_>>(),
        support.present_modes,
    );
    info!(
        "Swapchain: {:?} {:?}, {}x{}, {:?}, transform {:?}, composite alpha {:?}",
        format.format,
        format.color_space,
        extent.width,
        extent.height,
        present_mode,
        pre_transform,
        composite_alpha,
    );
}

impl Swapchain {
    /// Creates the swapchain for `data.surface`. With `log`, logs the surface's capabilities and
    /// the choices made (at start-up and on resume; not on every resize).
    pub unsafe fn create(
        window: &Window,
        instance: &Instance,
        device: &Device,
        data: &AppData,
        log: bool,
    ) -> Result<Self> {
        // Image

        let indices = QueueFamilyIndices::get(instance, data.surface, data.physical_device)?;
        let support = SwapchainSupport::get(instance, data.surface, data.physical_device)?;

        let window_size = (window.inner_size().width, window.inner_size().height);
        let surface_format = choose_surface_format(&support.formats);
        let present_mode = choose_present_mode(&support.present_modes);
        let composite_alpha = choose_composite_alpha(support.capabilities.supported_composite_alpha);
        let pre_transform = choose_pre_transform(&support.capabilities);
        let extent = choose_extent(&support.capabilities, pre_transform, window_size);
        if log {
            log_choices(
                &support,
                surface_format,
                present_mode,
                composite_alpha,
                pre_transform,
                extent,
                window_size,
            );
        }

        let swapchain_format = surface_format.format;
        let swapchain_extent = extent;

        let mut image_count = support.capabilities.min_image_count + 1;
        if support.capabilities.max_image_count != 0
            && image_count > support.capabilities.max_image_count
        {
            image_count = support.capabilities.max_image_count;
        }

        let mut queue_family_indices = vec![];
        let image_sharing_mode = if indices.graphics != indices.present {
            queue_family_indices.push(indices.graphics);
            queue_family_indices.push(indices.present);
            vk::SharingMode::CONCURRENT
        } else {
            vk::SharingMode::EXCLUSIVE
        };

        // Create

        let info = vk::SwapchainCreateInfoKHR::builder()
            .surface(data.surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(image_sharing_mode)
            .queue_family_indices(&queue_family_indices)
            .pre_transform(pre_transform)
            .composite_alpha(composite_alpha)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(vk::SwapchainKHR::null());

        let swapchain = device.create_swapchain_khr(&info, None)?;

        // Images

        let swapchain_images = device.get_swapchain_images_khr(swapchain)?;

        Ok(Self {
            swapchain_format,
            swapchain_extent,
            swapchain,
            swapchain_images,
            swapchain_image_views: vec![],
            suboptimal_expected: pre_transform != support.capabilities.current_transform,
        })
    }

    pub unsafe fn create_image_views(&mut self, device: &Device) -> Result<()> {
        self.swapchain_image_views = self
            .swapchain_images
            .iter()
            .map(|i| {
                create_image_view(
                    device,
                    *i,
                    self.swapchain_format,
                    vk::ImageAspectFlags::COLOR,
                    1,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }

    pub fn destroy(&mut self, device: &Device) {
        unsafe {
            for image_view in &self.swapchain_image_views {
                device.destroy_image_view(*image_view, None);
            }

            device.destroy_swapchain_khr(self.swapchain, None);
            self.swapchain_image_views.clear();
            self.swapchain = vk::SwapchainKHR::null();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(format: vk::Format) -> vk::SurfaceFormatKHR {
        vk::SurfaceFormatKHR {
            format,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        }
    }

    fn capabilities(
        current: (u32, u32),
        current_transform: vk::SurfaceTransformFlagsKHR,
        supported_transforms: vk::SurfaceTransformFlagsKHR,
    ) -> vk::SurfaceCapabilitiesKHR {
        vk::SurfaceCapabilitiesKHR {
            current_extent: vk::Extent2D { width: current.0, height: current.1 },
            min_image_extent: vk::Extent2D { width: 1, height: 1 },
            max_image_extent: vk::Extent2D { width: 4096, height: 4096 },
            current_transform,
            supported_transforms,
            ..Default::default()
        }
    }

    #[test]
    fn bgra_srgb_is_preferred() {
        let formats = [
            format(vk::Format::R8G8B8A8_SRGB),
            format(vk::Format::B8G8R8A8_SRGB),
        ];
        assert_eq!(choose_surface_format(&formats).format, vk::Format::B8G8R8A8_SRGB);
    }

    #[test]
    fn rgba_srgb_is_taken_over_unorm() {
        // What Android drivers typically offer.
        let formats = [
            format(vk::Format::R8G8B8A8_UNORM),
            format(vk::Format::R8G8B8A8_SRGB),
            format(vk::Format::R5G6B5_UNORM_PACK16),
        ];
        assert_eq!(choose_surface_format(&formats).format, vk::Format::R8G8B8A8_SRGB);
    }

    #[test]
    fn first_format_without_srgb() {
        let formats = [format(vk::Format::R8G8B8A8_UNORM)];
        assert_eq!(choose_surface_format(&formats).format, vk::Format::R8G8B8A8_UNORM);
    }

    #[test]
    fn composite_alpha_prefers_opaque_then_inherit() {
        let all = vk::CompositeAlphaFlagsKHR::OPAQUE | vk::CompositeAlphaFlagsKHR::INHERIT;
        assert_eq!(choose_composite_alpha(all), vk::CompositeAlphaFlagsKHR::OPAQUE);
        assert_eq!(
            choose_composite_alpha(vk::CompositeAlphaFlagsKHR::INHERIT),
            vk::CompositeAlphaFlagsKHR::INHERIT
        );
    }

    #[test]
    fn identity_transform_when_supported() {
        let rotated = capabilities(
            (1080, 2400),
            vk::SurfaceTransformFlagsKHR::ROTATE_90,
            vk::SurfaceTransformFlagsKHR::IDENTITY | vk::SurfaceTransformFlagsKHR::ROTATE_90,
        );
        assert_eq!(choose_pre_transform(&rotated), vk::SurfaceTransformFlagsKHR::IDENTITY);
        let fixed = capabilities(
            (1080, 2400),
            vk::SurfaceTransformFlagsKHR::ROTATE_90,
            vk::SurfaceTransformFlagsKHR::ROTATE_90,
        );
        assert_eq!(choose_pre_transform(&fixed), vk::SurfaceTransformFlagsKHR::ROTATE_90);
    }

    #[test]
    fn extent_is_the_surface_size_when_not_rotated() {
        let caps = capabilities(
            (1024, 768),
            vk::SurfaceTransformFlagsKHR::IDENTITY,
            vk::SurfaceTransformFlagsKHR::IDENTITY,
        );
        let extent = choose_extent(&caps, vk::SurfaceTransformFlagsKHR::IDENTITY, (1000, 700));
        assert_eq!((extent.width, extent.height), (1024, 768));
    }

    #[test]
    fn extent_is_the_window_size_when_rotated_by_the_display() {
        // A portrait phone showing a landscape activity, drawn without pre-rotation.
        let caps = capabilities(
            (1080, 2400),
            vk::SurfaceTransformFlagsKHR::ROTATE_90,
            vk::SurfaceTransformFlagsKHR::IDENTITY | vk::SurfaceTransformFlagsKHR::ROTATE_90,
        );
        let extent = choose_extent(&caps, vk::SurfaceTransformFlagsKHR::IDENTITY, (2400, 1080));
        assert_eq!((extent.width, extent.height), (2400, 1080));
    }

    #[test]
    fn extent_is_the_window_size_when_the_surface_has_none() {
        let caps = capabilities(
            (u32::MAX, u32::MAX),
            vk::SurfaceTransformFlagsKHR::IDENTITY,
            vk::SurfaceTransformFlagsKHR::IDENTITY,
        );
        let extent = choose_extent(&caps, vk::SurfaceTransformFlagsKHR::IDENTITY, (8000, 600));
        assert_eq!((extent.width, extent.height), (4096, 600));
    }
}
