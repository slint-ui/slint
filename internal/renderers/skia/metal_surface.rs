// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::{PhysicalSize as PhysicalWindowSize, Window};
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::item_rendering::DirtyRegion;
use objc2::rc::autoreleasepool;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::CGSize;
use objc2_metal::{MTLCommandBuffer, MTLCommandQueue, MTLDevice, MTLPixelFormat};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

use skia_safe::gpu::mtl;

use std::cell::RefCell;
use std::rc::Rc;

/// This surface renders into the given window using Metal. The provided display argument
/// is ignored, as it has no meaning on macOS.
pub struct MetalSurface {
    command_queue: Retained<ProtocolObject<dyn objc2_metal::MTLCommandQueue>>,
    layer: raw_window_metal::Layer,
    gr_context: RefCell<skia_safe::gpu::DirectContext>,
}

impl super::Surface for MetalSurface {
    fn new(
        window_handle: Rc<dyn raw_window_handle::HasWindowHandle>,
        _display_handle: Rc<dyn raw_window_handle::HasDisplayHandle>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
        if requested_graphics_api.map_or(false, |api| api != RequestedGraphicsAPI::Metal) {
            return Err(format!("Requested non-Metal rendering with Metal renderer").into());
        }

        let layer = match window_handle
            .window_handle()
            .map_err(|e| format!("Error obtaining window handle for skia metal renderer: {e}"))?
            .as_raw()
        {
            raw_window_handle::RawWindowHandle::AppKit(handle) => unsafe {
                raw_window_metal::Layer::from_ns_view(handle.ns_view)
            },
            raw_window_handle::RawWindowHandle::UiKit(handle) => unsafe {
                raw_window_metal::Layer::from_ui_view(handle.ui_view)
            },
            _ => return Err("Skia Renderer: Metal surface is only supported with AppKit".into()),
        };

        // SAFETY: The pointer is a valid `CAMetalLayer`.
        let ca_layer: &CAMetalLayer = unsafe { layer.as_ptr().cast().as_ref() };

        let device = {
            let ptr = unsafe { objc2_metal::MTLCreateSystemDefaultDevice() };
            unsafe { Retained::retain(ptr) }
                .ok_or_else(|| format!("Skia Renderer: No metal device found"))?
        };

        unsafe {
            ca_layer.setDevice(Some(&device));
            ca_layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
            ca_layer.setOpaque(false);
            ca_layer.setPresentsWithTransaction(false);

            ca_layer.setDrawableSize(CGSize::new(size.width as f64, size.height as f64));
        }

        let flipped = ca_layer.contentsAreFlipped();
        let gravity = if !flipped {
            unsafe { objc2_quartz_core::kCAGravityTopLeft }
        } else {
            unsafe { objc2_quartz_core::kCAGravityBottomLeft }
        };
        ca_layer.setContentsGravity(gravity);

        let command_queue = device
            .newCommandQueue()
            .ok_or_else(|| format!("Skia Renderer: Unable to create command queue"))?;

        let backend = unsafe {
            mtl::BackendContext::new(
                Retained::as_ptr(&device) as mtl::Handle,
                Retained::as_ptr(&command_queue) as mtl::Handle,
            )
        };

        let gr_context =
            skia_safe::gpu::direct_contexts::make_metal(&backend, None).unwrap().into();

        Ok(Self { command_queue, layer, gr_context })
    }

    fn name(&self) -> &'static str {
        "metal"
    }

    fn resize_event(
        &self,
        size: PhysicalWindowSize,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        // SAFETY: The pointer is a valid `CAMetalLayer`.
        let ca_layer: &CAMetalLayer = unsafe { self.layer.as_ptr().cast().as_ref() };
        unsafe {
            ca_layer.setDrawableSize(CGSize::new(size.width as f64, size.height as f64));
        }
        Ok(())
    }

    fn render(
        &self,
        _window: &Window,
        _size: PhysicalWindowSize,
        callback: &dyn Fn(
            &skia_safe::Canvas,
            Option<&mut skia_safe::gpu::DirectContext>,
            u8,
        ) -> Option<DirtyRegion>,
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        autoreleasepool(|_| {
            // SAFETY: The pointer is a valid `CAMetalLayer`.
            let ca_layer: &CAMetalLayer = unsafe { self.layer.as_ptr().cast().as_ref() };
            let drawable = match unsafe { ca_layer.nextDrawable() } {
                Some(drawable) => drawable,
                None => {
                    return Err(format!(
                        "Skia Metal Renderer: Failed to retrieve next drawable for rendering"
                    )
                    .into())
                }
            };

            let gr_context = &mut self.gr_context.borrow_mut();

            let size = unsafe { ca_layer.drawableSize() };

            let mut surface = unsafe {
                let texture = drawable.texture();
                let texture_info = mtl::TextureInfo::new(Retained::as_ptr(&texture) as mtl::Handle);

                let backend_render_target = skia_safe::gpu::backend_render_targets::make_mtl(
                    (size.width as i32, size.height as i32),
                    &texture_info,
                );

                skia_safe::gpu::surfaces::wrap_backend_render_target(
                    gr_context,
                    &backend_render_target,
                    skia_safe::gpu::SurfaceOrigin::TopLeft,
                    skia_safe::ColorType::BGRA8888,
                    None,
                    None,
                )
                .unwrap()
            };

            callback(surface.canvas(), Some(gr_context), 0);

            drop(surface);

            gr_context.submit(None);

            if let Some(pre_present_callback) = pre_present_callback.borrow_mut().as_mut() {
                pre_present_callback();
            }

            let command_buffer = self.command_queue.commandBuffer().ok_or_else(|| {
                format!("Skia Renderer: Unable to obtain command queue's command buffer")
            })?;
            command_buffer.presentDrawable(ProtocolObject::from_ref(&*drawable));
            command_buffer.commit();

            Ok(())
        })
    }

    fn bits_per_pixel(&self) -> Result<u8, i_slint_core::platform::PlatformError> {
        // SAFETY: The pointer is a valid `CAMetalLayer`.
        let ca_layer: &CAMetalLayer = unsafe { self.layer.as_ptr().cast().as_ref() };

        // From https://developer.apple.com/documentation/metal/mtlpixelformat:
        // The storage size of each pixel format is determined by the sum of its components.
        // For example, the storage size of BGRA8Unorm is 32 bits (four 8-bit components) and
        // the storage size of BGR5A1Unorm is 16 bits (three 5-bit components and one 1-bit component).
        Ok(match unsafe { ca_layer.pixelFormat() } {
            MTLPixelFormat::B5G6R5Unorm
            | MTLPixelFormat::A1BGR5Unorm
            | MTLPixelFormat::ABGR4Unorm
            | MTLPixelFormat::BGR5A1Unorm => 16,
            MTLPixelFormat::RGBA8Unorm
            | MTLPixelFormat::RGBA8Unorm_sRGB
            | MTLPixelFormat::RGBA8Snorm
            | MTLPixelFormat::RGBA8Uint
            | MTLPixelFormat::RGBA8Sint
            | MTLPixelFormat::BGRA8Unorm
            | MTLPixelFormat::BGRA8Unorm_sRGB => 32,
            MTLPixelFormat::RGB10A2Unorm
            | MTLPixelFormat::RGB10A2Uint
            | MTLPixelFormat::BGR10A2Unorm => 32,
            MTLPixelFormat::RGBA16Unorm
            | MTLPixelFormat::RGBA16Snorm
            | MTLPixelFormat::RGBA16Uint
            | MTLPixelFormat::RGBA16Sint => 64,
            MTLPixelFormat::RGBA32Uint | MTLPixelFormat::RGBA32Sint => 128,
            fmt @ _ => {
                return Err(format!(
                    "Skia Metal Renderer: Unsupported layer pixel format found {fmt:?}"
                )
                .into())
            }
        })
    }
}
