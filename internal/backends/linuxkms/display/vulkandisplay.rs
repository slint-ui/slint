// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::platform::PlatformError;
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{DeviceExtensions, QueueFlags};
use vulkano::instance::{Instance, InstanceCreateFlags, InstanceCreateInfo, InstanceExtensions};
use vulkano::swapchain::Surface;
use vulkano::VulkanLibrary;

use std::sync::Arc;

use super::Presenter;

pub struct VulkanDisplay {
    pub physical_device: Arc<PhysicalDevice>,
    pub queue_family_index: u32,
    pub surface: Arc<Surface>,
    pub size: PhysicalWindowSize,
    pub presenter: Arc<dyn Presenter>,
}

pub fn create_vulkan_display() -> Result<VulkanDisplay, PlatformError> {
    let library = VulkanLibrary::new()
        .map_err(|load_err| format!("Error loading vulkan library: {load_err}"))?;

    let required_extensions = InstanceExtensions {
        khr_surface: true,
        khr_display: true,
        khr_get_surface_capabilities2: true,
        khr_get_physical_device_properties2: true,
        ..InstanceExtensions::empty()
    }
    .intersection(library.supported_extensions());

    let instance = Instance::new(
        library.clone(),
        InstanceCreateInfo {
            flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
            enabled_extensions: required_extensions,
            ..Default::default()
        },
    )
    .map_err(|instance_err| format!("Error creating Vulkan instance: {instance_err}"))?;

    let device_extensions = DeviceExtensions { khr_swapchain: true, ..DeviceExtensions::empty() };
    let (physical_device, queue_family_index) = instance
        .enumerate_physical_devices()
        .map_err(|vke| format!("Error enumerating physical Vulkan devices: {vke}"))?
        .filter(|p| p.supported_extensions().contains(&device_extensions))
        .filter_map(|p| {
            p.queue_family_properties()
                .iter()
                .position(|q| {
                    q.queue_flags.intersects(QueueFlags::GRAPHICS)
                        && p.display_properties().map_or(false, |displays| !displays.is_empty())
                })
                .map(|i| (p, i as u32))
        })
        .min_by_key(|(p, _)| match p.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 0,
            PhysicalDeviceType::IntegratedGpu => 1,
            PhysicalDeviceType::VirtualGpu => 2,
            PhysicalDeviceType::Cpu => 3,
            PhysicalDeviceType::Other => 4,
            _ => 5,
        })
        .ok_or_else(|| format!("Vulkan: Failed to find suitable physical device"))?;

    let displays =
        physical_device.display_properties().map_err(|e| format!("Error reading displays: {e}"))?;

    let displays = displays.into_iter();

    let Some(first_display) = displays.clone().next() else {
        return Err(format!("Vulkan: No displays found").into());
    };

    let display = std::env::var("SLINT_VULKAN_DISPLAY").map_or_else(
        |_| Ok(first_display),
        |display_str| {
            let mut displays_and_index = displays.enumerate();

            if display_str.to_lowercase() == "list" {
                let display_names: Vec<String> = displays_and_index
                    .map(|(index, display)| {
                        format!(
                            "Index: {} Name: {}",
                            index,
                            display.name().unwrap_or_else(|| "unknown")
                        )
                    })
                    .collect();

                // Can't return error here because newlines are escaped.
                eprintln!("\nVulkan Display List Requested:\n{}\nPlease select a display with the SLINT_VULKAN_DISPLAY environment variable and re-run the program.", display_names.join("\n"));
                std::process::exit(1);
            }
            let display_index: usize =
                display_str.parse().map_err(|_| format!("Invalid display index {display_str}"))?;
            displays_and_index.nth(display_index).map_or_else(
                || Err(format!("Display index is out of bounds: {display_index}")),
                |(_, dsp)| Ok(dsp),
            )
        },
    )?;

    let mode = std::env::var("SLINT_VULKAN_MODE").map_or_else(
        |_| {
            display
                .display_mode_properties()
                .map_err(|e| format!("Error reading display mode properties: {e}"))?
                .into_iter()
                .max_by(|current_mode, next_mode| {
                    let [current_mode_width, current_mode_height] = current_mode.visible_region();
                    let current_refresh_rate = current_mode.refresh_rate();
                    let [next_mode_width, next_mode_height] = next_mode.visible_region();
                    let next_refresh_rate = next_mode.refresh_rate();
                    (current_mode_width, current_mode_height, current_refresh_rate).cmp(&(
                        next_mode_width,
                        next_mode_height,
                        next_refresh_rate,
                    ))
                })
                .ok_or_else(|| format!("Vulkan: No modes found for display"))
        },
        |mode_str| {
            let mut modes_and_index = display
                .display_mode_properties()
                .expect("fatal: Unable to enumerate display properties")
                .into_iter()
                .enumerate();

            if mode_str.to_lowercase() == "list" {
                let mode_names: Vec<String> = modes_and_index
                    .map(|(index, mode)| {
                        let [width, height] = mode.visible_region();
                        format!(
                            "Index: {index} Width: {width} Height: {height} Refresh Rate: {}",
                            mode.refresh_rate() / 1000
                        )
                    })
                    .collect();

                // Can't return error here because newlines are escaped.
                eprintln!("\nVulkan Mode List Requested:\n{}\nPlease select a mode with the SLINT_VULKAN_MODE environment variable and re-run the program.", mode_names.join("\n"));
                std::process::exit(1);
            }
            let mode_index: usize =
                mode_str.parse().map_err(|_| format!("Invalid mode index {mode_str}"))?;
            modes_and_index.nth(mode_index).map_or_else(
                || Err(format!("Mode index is out of bounds: {mode_index}")),
                |(_, mode)| Ok(mode),
            )
        },
    )?;

    let vulkan_surface = vulkano::swapchain::Surface::from_display_plane(
        mode.clone(),
        vulkano::swapchain::DisplaySurfaceCreateInfo {
            image_extent: [mode.visible_region()[0], mode.visible_region()[1]],
            ..Default::default()
        },
    )
    .unwrap();

    let size = PhysicalWindowSize::new(mode.visible_region()[0], mode.visible_region()[1]);

    Ok(VulkanDisplay {
        physical_device,
        queue_family_index,
        surface: vulkan_surface,
        size,
        presenter: crate::display::noop_presenter::NoopPresenter::new(),
    })
}
