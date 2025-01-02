use anyhow::Result;
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
}

fn get_swapchain_surface_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
    formats
        .iter()
        .cloned()
        .find(|f| {
            f.format == vk::Format::B8G8R8A8_SRGB
                && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
        })
        .unwrap_or_else(|| formats[0])
}

fn get_swapchain_present_mode(present_modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    present_modes
        .iter()
        .cloned()
        .find(|m| *m == vk::PresentModeKHR::MAILBOX)
        .unwrap_or(vk::PresentModeKHR::FIFO)
}

#[rustfmt::skip]
fn get_swapchain_extent(window: &Window, capabilities: vk::SurfaceCapabilitiesKHR) -> vk::Extent2D {
    if capabilities.current_extent.width != u32::MAX {
        capabilities.current_extent
    } else {
        vk::Extent2D::builder()
            .width(window.inner_size().width.clamp(
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width,
            ))
            .height(window.inner_size().height.clamp(
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height,
            ))
            .build()
    }
}

impl Swapchain {
    pub unsafe fn create(
        window: &Window,
        instance: &Instance,
        device: &Device,
        data: &AppData,
    ) -> Result<Self> {
        // Image

        let indices = QueueFamilyIndices::get(instance, data.surface, data.physical_device)?;
        let support = SwapchainSupport::get(instance, data.surface, data.physical_device)?;

        let surface_format = get_swapchain_surface_format(&support.formats);
        let present_mode = get_swapchain_present_mode(&support.present_modes);
        let extent = get_swapchain_extent(window, support.capabilities);

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
            .pre_transform(support.capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
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
}
