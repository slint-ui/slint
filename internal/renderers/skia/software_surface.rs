// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::{PhysicalSize as PhysicalWindowSize, Window};
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::item_rendering::DirtyRegion;
use i_slint_core::lengths::ScaleFactor;

use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;

use crate::PhysicalRect;

pub trait RenderBuffer {
    fn with_buffer(
        &self,
        window: &Window,
        size: PhysicalWindowSize,
        render_callback: &mut dyn FnMut(
            NonZeroU32,
            NonZeroU32,
            skia_safe::ColorType,
            u8,
            &mut [u8],
        ) -> Result<
            Option<DirtyRegion>,
            i_slint_core::platform::PlatformError,
        >,
    ) -> Result<(), i_slint_core::platform::PlatformError>;
}

struct SoftbufferRenderBuffer {
    _context: softbuffer::Context<Arc<dyn raw_window_handle::HasDisplayHandle>>,
    surface: RefCell<
        softbuffer::Surface<
            Arc<dyn raw_window_handle::HasDisplayHandle>,
            Arc<dyn raw_window_handle::HasWindowHandle>,
        >,
    >,
}

impl RenderBuffer for SoftbufferRenderBuffer {
    fn with_buffer(
        &self,
        window: &Window,
        size: PhysicalWindowSize,
        render_callback: &mut dyn FnMut(
            NonZeroU32,
            NonZeroU32,
            skia_safe::ColorType,
            u8,
            &mut [u8],
        ) -> Result<
            Option<DirtyRegion>,
            i_slint_core::platform::PlatformError,
        >,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let Some((width, height)) = size.width.try_into().ok().zip(size.height.try_into().ok())
        else {
            // Nothing to render
            return Ok(());
        };

        let mut surface = self.surface.borrow_mut();

        surface
            .resize(width, height)
            .map_err(|e| format!("Error resizing softbuffer surface: {e}"))?;

        let mut target_buffer = surface
            .buffer_mut()
            .map_err(|e| format!("Error retrieving softbuffer rendering buffer: {e}"))?;

        let dirty_region = render_callback(
            width,
            height,
            skia_safe::ColorType::BGRA8888,
            target_buffer.age(),
            bytemuck::cast_slice_mut(target_buffer.as_mut()),
        )?;

        if let Some(dirty_region) = dirty_region {
            let scale_factor = ScaleFactor::new(window.scale_factor());

            let damage_rects = dirty_region
                .iter()
                .map(|logical| {
                    let physical_rect: PhysicalRect =
                        (logical.to_rect() * scale_factor).round_out();
                    softbuffer::Rect {
                        x: physical_rect.min_x().ceil() as _,
                        y: physical_rect.min_y().ceil() as _,
                        width: ((physical_rect.width() as i32).max(1) as u32).try_into().unwrap(),
                        height: ((physical_rect.height() as i32).max(1) as u32).try_into().unwrap(),
                    }
                })
                .collect::<Vec<_>>();
            target_buffer.present_with_damage(&damage_rects)
        } else {
            target_buffer.present()
        }
        .map_err(|e| format!("Error presenting softbuffer buffer after skia rendering: {e}"))?;

        Ok(())
    }
}

/// This surface renders into the given window using Skia's software rasterize.
pub struct SoftwareSurface {
    render_buffer: Box<dyn RenderBuffer>,
}

impl super::Surface for SoftwareSurface {
    fn new(
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle>,
        display_handle: Arc<dyn raw_window_handle::HasDisplayHandle>,
        _size: PhysicalWindowSize,
        _requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
        let _context = softbuffer::Context::new(display_handle)
            .map_err(|e| format!("Error creating softbuffer context: {e}"))?;

        let surface =
            softbuffer::Surface::new(&_context, window_handle).map_err(|softbuffer_error| {
                format!("Error creating softbuffer surface: {softbuffer_error}")
            })?;

        let surface_access =
            Box::new(SoftbufferRenderBuffer { _context, surface: RefCell::new(surface) });

        Ok(Self { render_buffer: surface_access })
    }

    fn name(&self) -> &'static str {
        "software"
    }

    fn resize_event(
        &self,
        _size: PhysicalWindowSize,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        Ok(())
    }

    fn render(
        &self,
        window: &Window,
        size: PhysicalWindowSize,
        callback: &dyn Fn(
            &skia_safe::Canvas,
            Option<&mut skia_safe::gpu::DirectContext>,
            u8,
        ) -> Option<DirtyRegion>,
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.render_buffer.with_buffer(
            window,
            size,
            &mut |width, height, pixel_format, age, pixels| {
                let mut surface_borrow = skia_safe::surfaces::wrap_pixels(
                    &skia_safe::ImageInfo::new(
                        (width.get() as i32, height.get() as i32),
                        pixel_format,
                        skia_safe::AlphaType::Opaque,
                        None,
                    ),
                    pixels,
                    None,
                    None,
                )
                .ok_or_else(|| {
                    "Error wrapping target buffer for rendering into with Skia".to_string()
                })?;

                let dirty_region = callback(surface_borrow.canvas(), None, age);

                if let Some(pre_present_callback) = pre_present_callback.borrow_mut().as_mut() {
                    pre_present_callback();
                }

                Ok(dirty_region)
            },
        )
    }

    fn bits_per_pixel(&self) -> Result<u8, i_slint_core::platform::PlatformError> {
        Ok(24)
    }

    fn use_partial_rendering(&self) -> bool {
        true
    }
}

impl<T: RenderBuffer + 'static> From<T> for SoftwareSurface {
    fn from(render_buffer: T) -> Self {
        Self { render_buffer: Box::new(render_buffer) }
    }
}

impl<T: RenderBuffer + 'static> RenderBuffer for Rc<T> {
    fn with_buffer(
        &self,
        window: &Window,
        size: PhysicalWindowSize,
        render_callback: &mut dyn FnMut(
            NonZeroU32,
            NonZeroU32,
            skia_safe::ColorType,
            u8,
            &mut [u8],
        ) -> Result<
            Option<DirtyRegion>,
            i_slint_core::platform::PlatformError,
        >,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.as_ref().with_buffer(window, size, render_callback)
    }
}
