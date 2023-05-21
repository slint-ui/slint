// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains the window adapter implementation to communicate between Slint and Vulkan + libinput

use std::cell::Cell;
use std::rc::Rc;

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::{platform::PlatformError, window::WindowAdapter};
use i_slint_renderer_skia::SkiaRenderer;
use vulkano::device::{physical::PhysicalDeviceType, DeviceExtensions, QueueFlags};
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::swapchain::display::{Display, DisplayPlane};
use vulkano::VulkanLibrary;

pub struct VulkanWindowAdapter {
    window: i_slint_core::api::Window,
    renderer: SkiaRenderer,
    needs_redraw: Cell<bool>,
}

impl WindowAdapter for VulkanWindowAdapter {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }
}

impl i_slint_core::window::WindowAdapterSealed for VulkanWindowAdapter {
    fn size(&self) -> i_slint_core::api::PhysicalSize {
        let swapchain = self
            .renderer
            .surface()
            .as_any()
            .downcast_ref::<i_slint_renderer_skia::vulkan_surface::VulkanSurface>()
            .unwrap()
            .swapchain();
        let size = swapchain.create_info().image_extent;
        i_slint_core::api::PhysicalSize::new(size[0], size[1])
    }

    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn request_redraw(&self) {
        self.needs_redraw.set(true)
    }
}

impl VulkanWindowAdapter {
    pub fn new() -> Result<Rc<Self>, PlatformError> {
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
                enabled_extensions: required_extensions,
                enumerate_portability: true,
                ..Default::default()
            },
        )
        .map_err(|instance_err| format!("Error creating Vulkan instance: {instance_err}"))?;

        let device_extensions =
            DeviceExtensions { khr_swapchain: true, ..DeviceExtensions::empty() };
        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .map_err(|vke| format!("Error enumerating physical Vulkan devices: {vke}"))?
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .position(|q| {
                        q.queue_flags.intersects(QueueFlags::GRAPHICS)
                            && Display::enumerate(p.clone()).next().is_some()
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

        let displays = Display::enumerate(physical_device.clone());

        let Some(first_display) = displays.clone().next() else {
            return Err(format!("Vulkan: No displays found").into());
        };

        let display = std::env::var("SLINT_VULKANFS_DISPLAY").map_or_else(
            |_| Ok(first_display),
            |display_str| {
                let mut displays_and_index = displays.enumerate();

                if display_str.to_lowercase() == "list" {
                    let display_names: Vec<String> = displays_and_index
                        .map(|(index, display)| {
                            format!("Index: {} Name: {}", index, display.name())
                        })
                        .collect();

                    // Can't return error here because newlines are escaped.
                    panic!("\nVulkan Display List Requested:\n{}\n", display_names.join("\n"));
                }
                let display_index: usize = display_str
                    .parse()
                    .map_err(|_| format!("Invalid display index {display_str}"))?;
                displays_and_index.nth(display_index).map_or_else(
                    || Err(format!("Display index is out of bounds: {display_index}")),
                    |(_, dsp)| Ok(dsp),
                )
            },
        )?;

        let mode = std::env::var("SLINT_VULKANFS_MODE").map_or_else(
            |_| {
                display
                    .display_modes()
                    .max_by(|current_mode, next_mode| {
                        let [current_mode_width, current_mode_height] =
                            current_mode.visible_region();
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
                let mut modes_and_index = display.display_modes().enumerate();

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
                    panic!("\nVulkan Mode List Requested:\n{}\n", mode_names.join("\n"));
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
            &mode,
            &DisplayPlane::enumerate(physical_device.clone()).next().unwrap(),
        )
        .unwrap();

        let size = PhysicalWindowSize::new(mode.visible_region()[0], mode.visible_region()[1]);

        let surface = i_slint_renderer_skia::vulkan_surface::VulkanSurface::from_surface(
            physical_device,
            queue_family_index,
            vulkan_surface.clone(),
            size,
        )?;

        Ok(Rc::<VulkanWindowAdapter>::new_cyclic(|self_weak| VulkanWindowAdapter {
            window: i_slint_core::api::Window::new(self_weak.clone()),
            renderer: SkiaRenderer::new_with_surface(surface),
            needs_redraw: Cell::new(true),
        }))
    }

    pub fn render_if_needed(&self) -> Result<(), PlatformError> {
        if self.needs_redraw.replace(false) {
            self.renderer.render(&self.window)?;
        }
        Ok(())
    }
}
