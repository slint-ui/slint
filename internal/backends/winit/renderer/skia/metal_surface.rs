// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use foreign_types::ForeignTypeRef;
use metal::MTLPixelFormat;
use objc::{rc::autoreleasepool, runtime::YES};

use skia_safe::gpu::mtl;

use std::cell::RefCell;
use winit::platform::macos::WindowExtMacOS;

pub struct MetalSurface {
    command_queue: metal::CommandQueue,
    layer: metal::MetalLayer,
    gr_context: RefCell<skia_safe::gpu::DirectContext>,
    window: winit::window::Window,
}

impl super::Surface for MetalSurface {
    const SUPPORTS_GRAPHICS_API: bool = false;

    fn new(window_builder: winit::window::WindowBuilder) -> Self {
        let window = crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).unwrap()
        });

        let device = metal::Device::system_default().expect("no metal device found");

        let layer = metal::MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        layer.set_presents_with_transaction(false);

        let size = window.inner_size();
        layer.set_drawable_size(CGSize::new(size.width as f64, size.height as f64));

        unsafe {
            let view = window.ns_view() as cocoa_id;
            view.setWantsLayer(YES);
            view.setLayer(layer.as_ref() as *const _ as _);
        }

        let command_queue = device.new_command_queue();

        let backend = unsafe {
            mtl::BackendContext::new(
                device.as_ptr() as mtl::Handle,
                command_queue.as_ptr() as mtl::Handle,
                std::ptr::null(),
            )
        };

        let gr_context = skia_safe::gpu::DirectContext::new_metal(&backend, None).unwrap().into();

        Self { command_queue, layer, gr_context, window }
    }

    fn name(&self) -> &'static str {
        "metal"
    }

    fn window(&self) -> &winit::window::Window {
        &self.window
    }

    fn with_graphics_api(&self, _cb: impl FnOnce(i_slint_core::api::GraphicsAPI<'_>)) {
        unimplemented!()
    }

    fn resize_event(&self) {
        let size = self.window.inner_size();
        self.layer.set_drawable_size(CGSize::new(size.width as f64, size.height as f64));
    }

    fn render(
        &self,
        callback: impl FnOnce(&mut skia_safe::Canvas, &mut skia_safe::gpu::DirectContext),
    ) {
        autoreleasepool(|| {
            let drawable = match self.layer.next_drawable() {
                Some(drawable) => drawable,
                None => return,
            };

            let gr_context = &mut self.gr_context.borrow_mut();

            let size = self.layer.drawable_size();

            let mut surface = unsafe {
                let texture_info =
                    mtl::TextureInfo::new(drawable.texture().as_ptr() as mtl::Handle);

                let backend_render_target = skia_safe::gpu::BackendRenderTarget::new_metal(
                    (size.width as i32, size.height as i32),
                    1,
                    &texture_info,
                );

                skia_safe::Surface::from_backend_render_target(
                    gr_context,
                    &backend_render_target,
                    skia_safe::gpu::SurfaceOrigin::TopLeft,
                    skia_safe::ColorType::BGRA8888,
                    None,
                    None,
                )
                .unwrap()
            };

            callback(surface.canvas(), gr_context);

            drop(surface);

            gr_context.submit(None);

            let command_buffer = self.command_queue.new_command_buffer();
            command_buffer.present_drawable(drawable);
            command_buffer.commit();
        })
    }

    fn bits_per_pixel(&self) -> u8 {
        // From https://developer.apple.com/documentation/metal/mtlpixelformat:
        // The storage size of each pixel format is determined by the sum of its components.
        // For example, the storage size of BGRA8Unorm is 32 bits (four 8-bit components) and
        // the storage size of BGR5A1Unorm is 16 bits (three 5-bit components and one 1-bit component).
        match self.layer.pixel_format() {
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
            _ => 0, // Not mapped yet
        }
    }
}
