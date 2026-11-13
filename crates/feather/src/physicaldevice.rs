use std::collections::HashSet;

use anyhow::{anyhow, Result};
use log::*;
use thiserror::Error;
use vulkanalia::prelude::v1_0::*;

use super::appdata::AppData;
use super::queuefamilyindices::QueueFamilyIndices;
use super::swapchainsupport::SwapchainSupport;

/// The required device extensions.
const DEVICE_EXTENSIONS: &[vk::ExtensionName] = &[vk::KHR_SWAPCHAIN_EXTENSION.name];

//================================================
// Physical Device
//================================================

/// The optional device features the engine uses when the device has them. Desktop GPUs have
/// all of them; Android GPUs usually do, but not always.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeviceFeatures {
    /// Anisotropic texture filtering, on the texture sampler.
    pub sampler_anisotropy: bool,
    /// The largest anisotropy the sampler may use (`maxSamplerAnisotropy`).
    pub max_sampler_anisotropy: f32,
    /// Sample-rate shading: with MSAA, the fragment shader runs per sample, smoothing the
    /// inside of textured surfaces as well as their edges. Enabled only when the app asks for
    /// it (`RenderSettings::sample_shading`).
    pub sample_rate_shading: bool,
}

impl DeviceFeatures {
    pub unsafe fn get(instance: &Instance, physical_device: vk::PhysicalDevice) -> Self {
        let features = instance.get_physical_device_features(physical_device);
        let limits = instance.get_physical_device_properties(physical_device).limits;
        Self {
            sampler_anisotropy: features.sampler_anisotropy == vk::TRUE,
            max_sampler_anisotropy: limits.max_sampler_anisotropy,
            sample_rate_shading: features.sample_rate_shading == vk::TRUE,
        }
    }
}

#[derive(Debug, Error)]
#[error("{0}")]
pub struct SuitabilityError(pub &'static str);

pub unsafe fn pick_physical_device(instance: &Instance, data: &mut AppData) -> Result<()> {
    for physical_device in instance.enumerate_physical_devices()? {
        let properties = instance.get_physical_device_properties(physical_device);

        if let Err(error) = check_physical_device(instance, data, physical_device) {
            warn!(
                "Skipping physical device (`{}`): {}",
                properties.device_name, error
            );
        } else {
            info!("Selected physical device (`{}`).", properties.device_name);
            data.physical_device = physical_device;
            let settings = data.app.render_settings();
            data.msaa_samples = choose_msaa_samples(instance, data, settings.max_msaa_samples);
            data.features = DeviceFeatures::get(instance, physical_device);
            // Sample-rate shading only when the app asks for it (and there is MSAA to use it).
            data.features.sample_rate_shading &= settings.sample_shading
                && data.msaa_samples != vk::SampleCountFlags::_1;
            info!("MSAA samples: {:?}", data.msaa_samples);
            info!("Device features: {:?}", data.features);
            return Ok(());
        }
    }

    Err(anyhow!("Failed to find suitable physical device."))
}

unsafe fn check_physical_device(
    instance: &Instance,
    data: &AppData,
    physical_device: vk::PhysicalDevice,
) -> Result<()> {
    QueueFamilyIndices::get(instance, data.surface, physical_device)?;
    check_physical_device_extensions(instance, physical_device)?;

    let support = SwapchainSupport::get(instance, data.surface, physical_device)?;
    if support.formats.is_empty() || support.present_modes.is_empty() {
        return Err(anyhow!(SuitabilityError("Insufficient swapchain support.")));
    }

    Ok(())
}

unsafe fn check_physical_device_extensions(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
) -> Result<()> {
    let extensions = instance
        .enumerate_device_extension_properties(physical_device, None)?
        .iter()
        .map(|e| e.extension_name)
        .collect::<HashSet<_>>();
    if DEVICE_EXTENSIONS.iter().all(|e| extensions.contains(e)) {
        Ok(())
    } else {
        Err(anyhow!(SuitabilityError(
            "Missing required device extensions."
        )))
    }
}

/// The largest sample count, up to `max_samples`, that the device supports for both color and
/// depth attachments.
unsafe fn choose_msaa_samples(
    instance: &Instance,
    data: &AppData,
    max_samples: u32,
) -> vk::SampleCountFlags {
    let properties = instance.get_physical_device_properties(data.physical_device);
    let counts = properties.limits.framebuffer_color_sample_counts
        & properties.limits.framebuffer_depth_sample_counts;
    pick_sample_count(counts, max_samples)
}

/// The largest of `counts` that is at most `max_samples`, or 1.
fn pick_sample_count(counts: vk::SampleCountFlags, max_samples: u32) -> vk::SampleCountFlags {
    [
        (64, vk::SampleCountFlags::_64),
        (32, vk::SampleCountFlags::_32),
        (16, vk::SampleCountFlags::_16),
        (8, vk::SampleCountFlags::_8),
        (4, vk::SampleCountFlags::_4),
        (2, vk::SampleCountFlags::_2),
    ]
    .iter()
    .find(|(n, c)| *n <= max_samples && counts.contains(*c))
    .map(|(_, c)| *c)
    .unwrap_or(vk::SampleCountFlags::_1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_count_is_capped() {
        let all = vk::SampleCountFlags::_1
            | vk::SampleCountFlags::_2
            | vk::SampleCountFlags::_4
            | vk::SampleCountFlags::_8
            | vk::SampleCountFlags::_16;
        assert_eq!(pick_sample_count(all, 4), vk::SampleCountFlags::_4);
        assert_eq!(pick_sample_count(all, 64), vk::SampleCountFlags::_16);
        assert_eq!(pick_sample_count(all, 5), vk::SampleCountFlags::_4);
        assert_eq!(pick_sample_count(all, 1), vk::SampleCountFlags::_1);
        assert_eq!(pick_sample_count(all, 0), vk::SampleCountFlags::_1);
        let few = vk::SampleCountFlags::_1 | vk::SampleCountFlags::_2;
        assert_eq!(pick_sample_count(few, 4), vk::SampleCountFlags::_2);
    }
}
