// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use i_slint_core::api::{PhysicalSize as PhysicalWindowSize, Window};
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::item_rendering::DirtyRegion;

use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{
    Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo, QueueFlags,
};
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageUsage};
use vulkano::instance::{Instance, InstanceCreateFlags, InstanceCreateInfo, InstanceExtensions};
use vulkano::swapchain::{Surface, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo};
use vulkano::sync::GpuFuture;
use vulkano::{sync, Handle, Validated, VulkanError, VulkanLibrary, VulkanObject};

use crate::SkiaSharedContext;

pub struct SharedVulkanContext {
    instance: Arc<Instance>,
    // TODO: share also physical/logical device and queue, but their selection process is surface compatibility dependent.
}

impl super::SkiaSharedContextInner {
    fn shared_vulkan_context(
        &self,
    ) -> Result<&SharedVulkanContext, i_slint_core::platform::PlatformError> {
        if let Some(ctx) = self.vulkan_context.get() {
            return Ok(ctx);
        }
        self.vulkan_context.set(SharedVulkanContext::new()?).ok();
        Ok(self.vulkan_context.get().unwrap())
    }
}

impl SharedVulkanContext {
    fn new() -> Result<Self, i_slint_core::platform::PlatformError> {
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
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                enabled_extensions: required_extensions,
                ..Default::default()
            },
        )
        .map_err(|instance_err| format!("Error creating Vulkan instance: {instance_err}"))?;
        Ok(Self { instance })
    }
}

/// This surface renders into the given window using Vulkan.
pub struct VulkanSurface {
    gr_context: RefCell<skia_safe::gpu::DirectContext>,
    recreate_swapchain: Cell<bool>,
    device: Arc<Device>,
    previous_frame_end: RefCell<Option<Box<dyn GpuFuture>>>,
    queue: Arc<Queue>,
    swapchain: RefCell<Arc<Swapchain>>,
    swapchain_images: RefCell<Vec<Arc<Image>>>,
    swapchain_image_views: RefCell<Vec<Arc<ImageView>>>,
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
                .map_err(|vke| format!("Error matching Vulkan surface capabilities: {vke}"))?;
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

        let gr_context = skia_safe::gpu::direct_contexts::make_vulkan(&backend_context, None)
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

    /// Returns a clone of the shared swapchain.
    pub fn swapchain(&self) -> Arc<Swapchain> {
        self.swapchain.borrow().clone()
    }
}

impl super::Surface for VulkanSurface {
    fn new(
        shared_context: &SkiaSharedContext,
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle>,
        display_handle: Arc<dyn raw_window_handle::HasDisplayHandle>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
        if requested_graphics_api.map_or(false, |api| !matches!(api, RequestedGraphicsAPI::Vulkan))
        {
            return Err(format!("Requested non-Vulkan rendering with Vulkan renderer").into());
        }

        let instance = shared_context.0.shared_vulkan_context()?.instance.clone();

        let window_handle = window_handle
            .window_handle()
            .map_err(|e| format!("error obtaining window handle for skia vulkan renderer: {e}"))?;
        let display_handle = display_handle
            .display_handle()
            .map_err(|e| format!("error obtaining display handle for skia vulkan renderer: {e}"))?;

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
        _window: &Window,
        size: PhysicalWindowSize,
        callback: &dyn Fn(
            &skia_safe::Canvas,
            Option<&mut skia_safe::gpu::DirectContext>,
            u8,
        ) -> Option<DirtyRegion>,
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
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
            match vulkano::swapchain::acquire_next_image(swapchain.clone(), None)
                .map_err(Validated::unwrap)
            {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => {
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
        let height: i32 = height
            .try_into()
            .map_err(|_| format!("internal error: invalid swapchain image height {height}"))?;

        let image_view = self.swapchain_image_views.borrow()[image_index as usize].clone();
        let image_object = image_view.image();

        let format = image_view.format();

        debug_assert_eq!(format, vulkano::format::Format::B8G8R8A8_UNORM);
        let (vk_format, color_type) =
            (skia_safe::gpu::vk::Format::B8G8R8A8_UNORM, skia_safe::ColorType::BGRA8888);

        let alloc = skia_safe::gpu::vk::Alloc::default();
        let image_info = &unsafe {
            skia_safe::gpu::vk::ImageInfo::new(
                image_object.handle().as_raw() as _,
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
            &skia_safe::gpu::backend_render_targets::make_vk((width, height), image_info);

        let mut skia_surface = skia_safe::gpu::surfaces::wrap_backend_render_target(
            gr_context,
            render_target,
            skia_safe::gpu::SurfaceOrigin::TopLeft,
            color_type,
            None,
            None,
        )
        .ok_or_else(|| format!("Error creating Skia Vulkan surface"))?;

        callback(skia_surface.canvas(), Some(gr_context), 0);

        drop(skia_surface);

        gr_context.submit(None);

        if let Some(pre_present_callback) = pre_present_callback.borrow_mut().as_mut() {
            pre_present_callback();
        }

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

        match future.map_err(Validated::unwrap) {
            Ok(future) => {
                *self.previous_frame_end.borrow_mut() = Some(future.boxed());
            }
            Err(VulkanError::OutOfDate) => {
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

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

// FIXME(madsmtm): Why are we doing this instead of using `Surface::from_window`?
fn create_surface(
    instance: &Arc<Instance>,
    window_handle: raw_window_handle::WindowHandle<'_>,
    display_handle: raw_window_handle::DisplayHandle<'_>,
) -> Result<Arc<Surface>, vulkano::Validated<vulkano::VulkanError>> {
    match (window_handle.as_raw(), display_handle.as_raw()) {
        #[cfg(target_vendor = "apple")]
        (raw_window_handle::RawWindowHandle::AppKit(handle), _) => unsafe {
            let layer = raw_window_metal::Layer::from_ns_view(handle.ns_view);
            Surface::from_metal(instance.clone(), layer.as_ptr().as_ptr(), None)
        },
        #[cfg(target_vendor = "apple")]
        (raw_window_handle::RawWindowHandle::UiKit(handle), _) => unsafe {
            let layer = raw_window_metal::Layer::from_ui_view(handle.ui_view);
            Surface::from_metal(instance.clone(), layer.as_ptr().as_ptr(), None)
        },
        (
            raw_window_handle::RawWindowHandle::Xlib(raw_window_handle::XlibWindowHandle {
                window,
                ..
            }),
            raw_window_handle::RawDisplayHandle::Xlib(display),
        ) => unsafe {
            Surface::from_xlib(instance.clone(), display.display.unwrap().as_ptr(), window, None)
        },
        (
            raw_window_handle::RawWindowHandle::Xcb(raw_window_handle::XcbWindowHandle {
                window,
                ..
            }),
            raw_window_handle::RawDisplayHandle::Xcb(raw_window_handle::XcbDisplayHandle {
                connection,
                ..
            }),
        ) => unsafe {
            Surface::from_xcb(instance.clone(), connection.unwrap().as_ptr(), window.get(), None)
        },
        (
            raw_window_handle::RawWindowHandle::Wayland(raw_window_handle::WaylandWindowHandle {
                surface,
                ..
            }),
            raw_window_handle::RawDisplayHandle::Wayland(raw_window_handle::WaylandDisplayHandle {
                display,
                ..
            }),
        ) => unsafe {
            Surface::from_wayland(instance.clone(), display.as_ptr(), surface.as_ptr(), None)
        },
        (
            raw_window_handle::RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle {
                hwnd,
                hinstance,
                ..
            }),
            _,
        ) => unsafe {
            Surface::from_win32(
                instance.clone(),
                hinstance.unwrap().get() as *const std::ffi::c_void,
                hwnd.get() as *const std::ffi::c_void,
                None,
            )
        },
        _ => unimplemented!(),
    }
}
