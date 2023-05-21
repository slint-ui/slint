// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;

use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{
    Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo, QueueFlags,
};
use vulkano::image::view::ImageView;
use vulkano::image::{ImageAccess, ImageUsage, ImageViewAbstract, SwapchainImage};
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::swapchain::{
    AcquireError, Surface, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo,
};
use vulkano::sync::{FlushError, GpuFuture};
use vulkano::{sync, Handle, VulkanLibrary, VulkanObject};

use raw_window_handle::HasRawDisplayHandle;
use raw_window_handle::HasRawWindowHandle;

/// This surface renders into the given window using Vulkan.
pub struct VulkanSurface {
    gr_context: RefCell<skia_safe::gpu::DirectContext>,
    recreate_swapchain: Cell<bool>,
    device: Arc<Device>,
    previous_frame_end: RefCell<Option<Box<dyn GpuFuture>>>,
    queue: Arc<Queue>,
    swapchain: RefCell<Arc<Swapchain>>,
    swapchain_images: RefCell<Vec<Arc<SwapchainImage>>>,
    swapchain_image_views: RefCell<Vec<Arc<ImageView<SwapchainImage>>>>,
}

impl VulkanSurface {
    /// Creates a Skia Vulkan rendering surface from the given Vukano device, queue family index, surface,
    /// and size.
    pub fn from_surface(
        physical_device: Arc<PhysicalDevice>,
        queue_family_index: u32,
        surface: Arc<Surface>,
        size: PhysicalWindowSize,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
        /*
        eprintln!(
            "Vulkan device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );*/

        let (device, mut queues) = Device::new(
            physical_device.clone(),
            DeviceCreateInfo {
                enabled_extensions: DeviceExtensions {
                    khr_swapchain: true,
                    ..DeviceExtensions::empty()
                },
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .map_err(|dev_err| format!("Failed to create suitable logical Vulkan device: {dev_err}"))?;
        let queue = queues.next().ok_or_else(|| format!("Not Vulkan device queue found"))?;

        let (swapchain, swapchain_images) = {
            let surface_capabilities = device
                .physical_device()
                .surface_capabilities(&surface, Default::default())
                .map_err(|vke| format!("Error macthing Vulkan surface capabilities: {vke}"))?;
            let image_format = vulkano::format::Format::B8G8R8A8_UNORM.into();

            Swapchain::new(
                device.clone(),
                surface.clone(),
                SwapchainCreateInfo {
                    min_image_count: surface_capabilities.min_image_count,
                    image_format,
                    image_extent: [size.width, size.height],
                    image_usage: ImageUsage::COLOR_ATTACHMENT,
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha
                        .into_iter()
                        .next()
                        .ok_or_else(|| format!("fatal: Vulkan surface capabilities missing composite alpha descriptor"))?,
                    ..Default::default()
                },
            )
            .map_err(|vke| format!("Error creating Vulkan swapchain: {vke}"))?
        };

        let mut swapchain_image_views = Vec::with_capacity(swapchain_images.len());

        for image in &swapchain_images {
            swapchain_image_views.push(ImageView::new_default(image.clone()).map_err(|vke| {
                format!("fatal: Error creating image view for swap chain image: {vke}")
            })?);
        }

        let instance = physical_device.instance();
        let library = instance.library();

        let get_proc = |of| unsafe {
            let result = match of {
                skia_safe::gpu::vk::GetProcOf::Instance(instance, name) => {
                    library.get_instance_proc_addr(ash::vk::Instance::from_raw(instance as _), name)
                }
                skia_safe::gpu::vk::GetProcOf::Device(device, name) => {
                    (instance.fns().v1_0.get_device_proc_addr)(
                        ash::vk::Device::from_raw(device as _),
                        name,
                    )
                }
            };

            match result {
                Some(f) => f as _,
                None => {
                    //println!("resolve of {} failed", of.name().to_str().unwrap());
                    core::ptr::null()
                }
            }
        };

        let backend_context = unsafe {
            skia_safe::gpu::vk::BackendContext::new(
                instance.handle().as_raw() as _,
                physical_device.handle().as_raw() as _,
                device.handle().as_raw() as _,
                (queue.handle().as_raw() as _, queue.id_within_family() as _),
                &get_proc,
            )
        };

        let gr_context = skia_safe::gpu::DirectContext::new_vulkan(&backend_context, None)
            .ok_or_else(|| format!("Error creating Skia Vulkan context"))?;

        let previous_frame_end = RefCell::new(Some(sync::now(device.clone()).boxed()));

        Ok(Self {
            gr_context: RefCell::new(gr_context),
            recreate_swapchain: Cell::new(false),
            device,
            previous_frame_end,
            queue,
            swapchain: RefCell::new(swapchain),
            swapchain_images: RefCell::new(swapchain_images),
            swapchain_image_views: RefCell::new(swapchain_image_views),
        })
    }
}

impl super::Surface for VulkanSurface {
    fn new(
        window_handle: raw_window_handle::WindowHandle<'_>,
        display_handle: raw_window_handle::DisplayHandle<'_>,
        size: PhysicalWindowSize,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
        let library = VulkanLibrary::new()
            .map_err(|load_err| format!("Error loading vulkan library: {load_err}"))?;

        let required_extensions = InstanceExtensions {
            khr_surface: true,
            mvk_macos_surface: true,
            ext_metal_surface: true,
            khr_wayland_surface: true,
            khr_xlib_surface: true,
            khr_xcb_surface: true,
            khr_win32_surface: true,
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

        let surface = create_surface(&instance, window_handle, display_handle)
            .map_err(|surface_err| format!("Error creating Vulkan surface: {surface_err}"))?;

        let device_extensions =
            DeviceExtensions { khr_swapchain: true, ..DeviceExtensions::empty() };
        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .map_err(|vke| format!("Error enumerating physical Vulkan devices: {vke}"))?
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        q.queue_flags.intersects(QueueFlags::GRAPHICS)
                            && p.surface_support(i as u32, &surface).unwrap_or(false)
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

        Self::from_surface(physical_device, queue_family_index, surface, size)
    }

    fn name(&self) -> &'static str {
        "vulkan"
    }

    fn resize_event(
        &self,
        _size: PhysicalWindowSize,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.recreate_swapchain.set(true);
        Ok(())
    }

    fn render(
        &self,
        size: PhysicalWindowSize,
        callback: &dyn Fn(&mut skia_safe::Canvas, &mut skia_safe::gpu::DirectContext),
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let gr_context = &mut self.gr_context.borrow_mut();

        let device = self.device.clone();

        self.previous_frame_end.borrow_mut().as_mut().unwrap().cleanup_finished();

        if self.recreate_swapchain.take() {
            let mut swapchain = self.swapchain.borrow_mut();
            let (new_swapchain, new_images) = swapchain
                .recreate(SwapchainCreateInfo {
                    image_extent: [size.width, size.height],
                    ..swapchain.create_info()
                })
                .map_err(|vke| format!("Error re-creating Vulkan swap chain: {vke}"))?;

            *swapchain = new_swapchain;

            let mut new_swapchain_image_views = Vec::with_capacity(new_images.len());

            for image in &new_images {
                new_swapchain_image_views.push(ImageView::new_default(image.clone()).map_err(
                    |vke| format!("fatal: Error creating image view for swap chain image: {vke}"),
                )?);
            }

            *self.swapchain_images.borrow_mut() = new_images;
            *self.swapchain_image_views.borrow_mut() = new_swapchain_image_views;
        }

        let swapchain = self.swapchain.borrow().clone();

        let (image_index, suboptimal, acquire_future) =
            match vulkano::swapchain::acquire_next_image(swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.recreate_swapchain.set(true);
                    return Ok(()); // Try again next frame
                }
                Err(e) => return Err(format!("Vulkan: failed to acquire next image: {e}").into()),
            };

        if suboptimal {
            self.recreate_swapchain.set(true);
        }

        let width = swapchain.image_extent()[0];
        let width: i32 = width
            .try_into()
            .map_err(|_| format!("internal error: invalid swapchain image width {width}"))?;
        let height = swapchain.image_extent()[1];
        let height: i32 = width
            .try_into()
            .map_err(|_| format!("internal error: invalid swapchain image height {height}"))?;

        let image_view = self.swapchain_image_views.borrow()[image_index as usize].clone();
        let image_object = image_view.image();

        let format = image_view.format();

        debug_assert_eq!(format, Some(vulkano::format::Format::B8G8R8A8_UNORM));
        let (vk_format, color_type) =
            (skia_safe::gpu::vk::Format::B8G8R8A8_UNORM, skia_safe::ColorType::BGRA8888);

        let alloc = skia_safe::gpu::vk::Alloc::default();
        let image_info = &unsafe {
            skia_safe::gpu::vk::ImageInfo::new(
                image_object.inner().image.handle().as_raw() as _,
                alloc,
                skia_safe::gpu::vk::ImageTiling::OPTIMAL,
                skia_safe::gpu::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk_format,
                1,
                None,
                None,
                None,
                None,
            )
        };

        let render_target =
            &skia_safe::gpu::BackendRenderTarget::new_vulkan((width, height), 0, image_info);

        let mut skia_surface = skia_safe::Surface::from_backend_render_target(
            gr_context,
            render_target,
            skia_safe::gpu::SurfaceOrigin::TopLeft,
            color_type,
            None,
            None,
        )
        .ok_or_else(|| format!("Error creating Skia Vulkan surface"))?;

        callback(skia_surface.canvas(), gr_context);

        drop(skia_surface);

        gr_context.submit(None);

        let future = self
            .previous_frame_end
            .borrow_mut()
            .take()
            .unwrap()
            .join(acquire_future)
            .then_swapchain_present(
                self.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(swapchain.clone(), image_index),
            )
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                *self.previous_frame_end.borrow_mut() = Some(future.boxed());
            }
            Err(FlushError::OutOfDate) => {
                self.recreate_swapchain.set(true);
                *self.previous_frame_end.borrow_mut() = Some(sync::now(device.clone()).boxed());
            }
            Err(e) => {
                *self.previous_frame_end.borrow_mut() = Some(sync::now(device.clone()).boxed());
                return Err(format!("Skia Vulkan renderer: failed to flush future: {e}").into());
            }
        }

        Ok(())
    }

    fn bits_per_pixel(&self) -> Result<u8, i_slint_core::platform::PlatformError> {
        Ok(match self.swapchain.borrow().image_format() {
            vulkano::format::Format::B8G8R8A8_UNORM => 32,
            fmt @ _ => {
                return Err(format!(
                    "Skia Vulkan Renderer: Unsupported swapchain image format found {fmt:?}"
                )
                .into())
            }
        })
    }
}

fn create_surface(
    instance: &Arc<Instance>,
    window_handle: raw_window_handle::WindowHandle<'_>,
    display_handle: raw_window_handle::DisplayHandle<'_>,
) -> Result<Arc<Surface>, vulkano::swapchain::SurfaceCreationError> {
    match (window_handle.raw_window_handle(), display_handle.raw_display_handle()) {
        #[cfg(target_os = "macos")]
        (
            raw_window_handle::RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle {
                ns_view,
                ..
            }),
            _,
        ) => unsafe {
            use cocoa::{appkit::NSView, base::id as cocoa_id};
            use objc::runtime::YES;

            let layer = metal::MetalLayer::new();
            layer.set_opaque(false);
            layer.set_presents_with_transaction(false);
            let view = ns_view as cocoa_id;
            view.setWantsLayer(YES);
            view.setLayer(layer.as_ref() as *const _ as _);
            Surface::from_metal(instance.clone(), layer.as_ref(), None)
        },
        (
            raw_window_handle::RawWindowHandle::Xlib(raw_window_handle::XlibWindowHandle {
                window,
                ..
            }),
            raw_window_handle::RawDisplayHandle::Xlib(display),
        ) => unsafe { Surface::from_xlib(instance.clone(), display.display, window, None) },
        (
            raw_window_handle::RawWindowHandle::Xcb(raw_window_handle::XcbWindowHandle {
                window,
                ..
            }),
            raw_window_handle::RawDisplayHandle::Xcb(raw_window_handle::XcbDisplayHandle {
                connection,
                ..
            }),
        ) => unsafe { Surface::from_xcb(instance.clone(), connection, window, None) },
        (
            raw_window_handle::RawWindowHandle::Wayland(raw_window_handle::WaylandWindowHandle {
                surface,
                ..
            }),
            raw_window_handle::RawDisplayHandle::Wayland(raw_window_handle::WaylandDisplayHandle {
                display,
                ..
            }),
        ) => unsafe { Surface::from_wayland(instance.clone(), display, surface, None) },
        (
            raw_window_handle::RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle {
                hwnd,
                hinstance,
                ..
            }),
            _,
        ) => unsafe { Surface::from_win32(instance.clone(), hinstance, hwnd, None) },
        _ => unimplemented!(),
    }
}
