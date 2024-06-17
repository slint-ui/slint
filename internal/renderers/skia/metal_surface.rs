// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use foreign_types::ForeignTypeRef;
use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use metal::MTLPixelFormat;
use objc::{rc::autoreleasepool, runtime::YES};

use skia_safe::gpu::mtl;

use std::cell::RefCell;
use std::rc::Rc;

/// This surface renders into the given window using Metal. The provided display argument
/// is ignored, as it has no meaning on macOS.
pub struct MetalSurface {
    command_queue: metal::CommandQueue,
    layer: metal::MetalLayer,
    gr_context: RefCell<skia_safe::gpu::DirectContext>,
}

impl super::Surface for MetalSurface {
    fn new(
        window_handle: Rc<dyn raw_window_handle::HasWindowHandle>,
        _display_handle: Rc<dyn raw_window_handle::HasDisplayHandle>,
        size: PhysicalWindowSize,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
        let device = metal::Device::system_default()
            .ok_or_else(|| format!("Skia Renderer: No metal device found"))?;

        let layer = metal::MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        layer.set_opaque(false);
        layer.set_presents_with_transaction(false);

        layer.set_drawable_size(CGSize::new(size.width as f64, size.height as f64));

        unsafe {
            let view = match window_handle
                .window_handle()
                .map_err(|e| format!("Error obtaining window handle for skia metal renderer: {e}"))?
                .as_raw()
            {
                raw_window_handle::RawWindowHandle::AppKit(
                    raw_window_handle::AppKitWindowHandle { ns_view, .. },
                ) => ns_view.as_ptr(),
                _ => {
                    return Err("Skia Renderer: Metal surface is only supported with AppKit".into())
                }
            } as cocoa_id;
            view.setWantsLayer(YES);
            view.setLayer(layer.as_ref() as *const _ as _);
            view.setLayerContentsPlacement(
                cocoa::appkit::NSViewLayerContentsPlacement::NSViewLayerContentsPlacementTopLeft,
            );
        }

        let command_queue = device.new_command_queue();

        let backend = unsafe {
            mtl::BackendContext::new(
                device.as_ptr() as mtl::Handle,
                command_queue.as_ptr() as mtl::Handle,
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
        self.layer.set_drawable_size(CGSize::new(size.width as f64, size.height as f64));
        Ok(())
    }

    fn set_scale_factor(&self, scale_factor: f32) {
        self.layer.set_contents_scale(scale_factor.into());
    }

    fn render(
        &self,
        _size: PhysicalWindowSize,
        callback: &dyn Fn(&skia_safe::Canvas, Option<&mut skia_safe::gpu::DirectContext>),
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        autoreleasepool(|| {
            let drawable = match self.layer.next_drawable() {
                Some(drawable) => drawable,
                None => {
                    return Err(format!(
                        "Skia Metal Renderer: Failed to retrieve next drawable for rendering"
                    )
                    .into())
                }
            };

            let gr_context = &mut self.gr_context.borrow_mut();

            let size = self.layer.drawable_size();

            let mut surface = unsafe {
                let texture_info =
                    mtl::TextureInfo::new(drawable.texture().as_ptr() as mtl::Handle);

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

            callback(surface.canvas(), Some(gr_context));

            drop(surface);

            gr_context.submit(None);

            if let Some(pre_present_callback) = pre_present_callback.borrow_mut().as_mut() {
                pre_present_callback();
            }

            let command_buffer = self.command_queue.new_command_buffer();
            command_buffer.present_drawable(drawable);
            command_buffer.commit();

            Ok(())
        })
    }

    fn bits_per_pixel(&self) -> Result<u8, i_slint_core::platform::PlatformError> {
        // From https://developer.apple.com/documentation/metal/mtlpixelformat:
        // The storage size of each pixel format is determined by the sum of its components.
        // For example, the storage size of BGRA8Unorm is 32 bits (four 8-bit components) and
        // the storage size of BGR5A1Unorm is 16 bits (three 5-bit components and one 1-bit component).
        Ok(match self.layer.pixel_format() {
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
