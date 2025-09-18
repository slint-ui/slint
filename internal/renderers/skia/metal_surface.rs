// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::{PhysicalSize as PhysicalWindowSize, Window};
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::item_rendering::DirtyRegion;
use objc2::rc::autoreleasepool;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_core_foundation::CGSize;
use objc2_metal::{MTLCommandBuffer, MTLCommandQueue, MTLDevice, MTLPixelFormat, MTLTexture};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

use skia_safe::gpu::mtl;

use std::cell::RefCell;
use std::sync::Arc;

use crate::SkiaSharedContext;

pub struct SharedMetalContext {
    device: Retained<ProtocolObject<dyn objc2_metal::MTLDevice>>,
    command_queue: Retained<ProtocolObject<dyn objc2_metal::MTLCommandQueue>>,
}

impl super::SkiaSharedContextInner {
    fn shared_metal_context(
        &self,
    ) -> Result<&SharedMetalContext, i_slint_core::platform::PlatformError> {
        if let Some(ctx) = self.metal_context.get() {
            return Ok(ctx);
        }
        self.metal_context.set(SharedMetalContext::new()?).ok();
        Ok(self.metal_context.get().unwrap())
    }
}

impl SharedMetalContext {
    fn new() -> Result<Self, i_slint_core::platform::PlatformError> {
        let device = objc2_metal::MTLCreateSystemDefaultDevice().ok_or_else(|| {
            format!("Skia Renderer: Unable to obtain metal system default device")
        })?;
        let command_queue = device
            .newCommandQueue()
            .ok_or_else(|| format!("Skia Renderer: Unable to create command queue"))?;
        Ok(Self { device, command_queue })
    }
}

/// This surface renders into the given window using Metal. The provided display argument
/// is ignored, as it has no meaning on macOS.
pub struct MetalSurface {
    command_queue: Retained<ProtocolObject<dyn objc2_metal::MTLCommandQueue>>,
    layer: raw_window_metal::Layer,
    gr_context: RefCell<skia_safe::gpu::DirectContext>,
    // Map from drawable texture to age. Per https://developer.apple.com/documentation/quartzcore/cametallayer/maximumdrawablecount, CAMetalLayer
    // can have either 2 or 3 drawables, but not more. That way, this vector is bound in growth.
    drawable_ages: RefCell<Vec<(objc2_metal::MTLResourceID, u8)>>,
}

impl super::Surface for MetalSurface {
    fn new(
        shared_context: &SkiaSharedContext,
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
        _display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
        if requested_graphics_api.map_or(false, |api| !matches!(api, RequestedGraphicsAPI::Metal)) {
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

        let shared_context = shared_context.0.shared_metal_context()?;

        let device = &shared_context.device;

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

        let command_queue = shared_context.command_queue.clone();

        let backend = unsafe {
            mtl::BackendContext::new(
                Retained::as_ptr(&device) as mtl::Handle,
                Retained::as_ptr(&command_queue) as mtl::Handle,
            )
        };

        let gr_context =
            skia_safe::gpu::direct_contexts::make_metal(&backend, None).unwrap().into();

        Ok(Self { command_queue, layer, gr_context, drawable_ages: Default::default() })
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
        self.drawable_ages.borrow_mut().clear();
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

            let texture: Retained<ProtocolObject<dyn MTLTexture>> = unsafe { drawable.texture() };
            let texture_id = unsafe { texture.gpuResourceID() };
            let age = {
                let mut drawables = self.drawable_ages.borrow_mut();
                if let Some(existing_age) =
                    drawables.iter().find_map(|(id, age)| (*id == texture_id).then_some(*age))
                {
                    existing_age
                } else {
                    drawables.push((texture_id, 0));
                    0
                }
            };
            callback(surface.canvas(), Some(gr_context), age);

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

            self.drawable_ages.borrow_mut().retain_mut(|(id, age)| {
                if *id == texture_id {
                    *age = 1;
                } else {
                    let Some(new_age) = age.checked_add(1) else {
                        // texture became too old, remove it.
                        return false;
                    };
                    *age = new_age;
                }
                true
            });

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
