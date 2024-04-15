// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;

use std::{cell::RefCell, num::NonZeroU32};

pub trait RenderBuffer {
    fn with_buffer(
        &self,
        size: PhysicalWindowSize,
        render_callback: &mut dyn FnMut(
            NonZeroU32,
            NonZeroU32,
            skia_safe::ColorType,
            &mut [u8],
        )
            -> Result<(), i_slint_core::platform::PlatformError>,
    ) -> Result<(), i_slint_core::platform::PlatformError>;
}

struct SoftbufferRenderBuffer {
    _context: softbuffer::Context,
    surface: RefCell<softbuffer::Surface>,
}

impl RenderBuffer for SoftbufferRenderBuffer {
    fn with_buffer(
        &self,
        size: PhysicalWindowSize,
        render_callback: &mut dyn FnMut(
            NonZeroU32,
            NonZeroU32,
            skia_safe::ColorType,
            &mut [u8],
        )
            -> Result<(), i_slint_core::platform::PlatformError>,
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

        render_callback(
            width,
            height,
            skia_safe::ColorType::BGRA8888,
            bytemuck::cast_slice_mut(target_buffer.as_mut()),
        )?;

        target_buffer
            .present()
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
        window_handle: raw_window_handle::WindowHandle<'_>,
        display_handle: raw_window_handle::DisplayHandle<'_>,
        _size: PhysicalWindowSize,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
        let _context = unsafe {
            softbuffer::Context::new(&display_handle)
                .map_err(|e| format!("Error creating softbuffer context: {e}"))?
        };

        let surface = unsafe { softbuffer::Surface::new(&_context, &window_handle) }.map_err(
            |softbuffer_error| format!("Error creating softbuffer surface: {}", softbuffer_error),
        )?;

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
        size: PhysicalWindowSize,
        callback: &dyn Fn(&skia_safe::Canvas, Option<&mut skia_safe::gpu::DirectContext>),
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.render_buffer.with_buffer(size, &mut |width, height, pixel_format, pixels| {
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
            .ok_or_else(|| format!("Error wrapping target buffer for rendering into with Skia"))?;

            callback(surface_borrow.canvas(), None);

            if let Some(pre_present_callback) = pre_present_callback.borrow_mut().as_mut() {
                pre_present_callback();
            }

            Ok(())
        })
    }

    fn bits_per_pixel(&self) -> Result<u8, i_slint_core::platform::PlatformError> {
        Ok(24)
    }
}

impl<T: RenderBuffer + 'static> From<T> for SoftwareSurface {
    fn from(render_buffer: T) -> Self {
        Self { render_buffer: Box::new(render_buffer) }
    }
}
