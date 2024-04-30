// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

//! Delegate the rendering to the [`i_slint_core::software_renderer::SoftwareRenderer`]

use core::num::NonZeroU32;
use core::ops::DerefMut;
use i_slint_core::graphics::Rgb8Pixel;
use i_slint_core::platform::PlatformError;
pub use i_slint_core::software_renderer::SoftwareRenderer;
use i_slint_core::software_renderer::{PremultipliedRgbaColor, RepaintBufferType, TargetPixel};
use std::{cell::RefCell, rc::Rc};

use super::WinitCompatibleRenderer;

pub struct WinitSoftwareRenderer {
    renderer: SoftwareRenderer,
    _context: softbuffer::Context,
    surface: RefCell<softbuffer::Surface>,
    winit_window: Rc<winit::window::Window>,
}

#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SoftBufferPixel(pub u32);

impl From<SoftBufferPixel> for PremultipliedRgbaColor {
    #[inline]
    fn from(pixel: SoftBufferPixel) -> Self {
        let v = pixel.0;
        PremultipliedRgbaColor {
            red: (v >> 16) as u8,
            green: (v >> 8) as u8,
            blue: (v >> 0) as u8,
            alpha: (v >> 24) as u8,
        }
    }
}

impl From<PremultipliedRgbaColor> for SoftBufferPixel {
    #[inline]
    fn from(pixel: PremultipliedRgbaColor) -> Self {
        Self(
            (pixel.alpha as u32) << 24
                | ((pixel.red as u32) << 16)
                | ((pixel.green as u32) << 8)
                | (pixel.blue as u32),
        )
    }
}

impl TargetPixel for SoftBufferPixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let mut x = PremultipliedRgbaColor::from(*self);
        x.blend(color);
        *self = x.into();
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(0xff000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    fn background() -> Self {
        Self(0)
    }
}

impl WinitSoftwareRenderer {
    pub fn new(
        window_builder: winit::window::WindowBuilder,
    ) -> Result<(Box<dyn WinitCompatibleRenderer>, Rc<winit::window::Window>), PlatformError> {
        let winit_window = crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).map_err(|winit_os_error| {
                format!("Error creating native window for software rendering: {}", winit_os_error)
                    .into()
            })
        })?;

        let context = unsafe {
            softbuffer::Context::new(&winit_window)
                .map_err(|e| format!("Error creating softbuffer context: {e}"))?
        };

        let surface = unsafe { softbuffer::Surface::new(&context, &winit_window) }.map_err(
            |softbuffer_error| format!("Error creating softbuffer surface: {}", softbuffer_error),
        )?;

        let winit_window = Rc::new(winit_window);

        Ok((
            Box::new(Self {
                renderer: SoftwareRenderer::new(),
                _context: context,
                surface: RefCell::new(surface),
                winit_window: winit_window.clone(),
            }),
            winit_window,
        ))
    }
}

impl super::WinitCompatibleRenderer for WinitSoftwareRenderer {
    fn render(&self, window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        let size = window.size();

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

        let age = target_buffer.age();
        self.renderer.set_repaint_buffer_type(match age {
            1 => RepaintBufferType::ReusedBuffer,
            2 => RepaintBufferType::SwappedBuffers,
            _ => RepaintBufferType::NewBuffer,
        });

        let region = if std::env::var_os("SLINT_LINE_BY_LINE").is_none() {
            let buffer: &mut [SoftBufferPixel] =
                bytemuck::cast_slice_mut(target_buffer.deref_mut());
            self.renderer.render(buffer, width.get() as usize)
        } else {
            // SLINT_LINE_BY_LINE is set and this is a debug mode where we also render in a Rgb565Pixel
            struct FrameBuffer<'a> {
                buffer: &'a mut [u32],
                line: Vec<i_slint_core::software_renderer::Rgb565Pixel>,
            }
            impl<'a> i_slint_core::software_renderer::LineBufferProvider for FrameBuffer<'a> {
                type TargetPixel = i_slint_core::software_renderer::Rgb565Pixel;
                fn process_line(
                    &mut self,
                    line: usize,
                    range: core::ops::Range<usize>,
                    render_fn: impl FnOnce(&mut [Self::TargetPixel]),
                ) {
                    let line_begin = line * self.line.len();
                    let sub = &mut self.line[..range.len()];
                    render_fn(sub);
                    for (dst, src) in self.buffer[line_begin..][range].iter_mut().zip(sub) {
                        let p = Rgb8Pixel::from(*src);
                        *dst =
                            0xff000000 | ((p.r as u32) << 16) | ((p.g as u32) << 8) | (p.b as u32);
                    }
                }
            }
            self.renderer.render_by_line(FrameBuffer {
                buffer: &mut target_buffer,
                line: vec![Default::default(); width.get() as usize],
            })
        };

        self.winit_window.pre_present_notify();

        let size = region.bounding_box_size();
        if let Some((w, h)) = Option::zip(NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        {
            let pos = region.bounding_box_origin();
            target_buffer
                .present_with_damage(&[softbuffer::Rect {
                    width: w,
                    height: h,
                    x: pos.x as u32,
                    y: pos.y as u32,
                }])
                .map_err(|e| format!("Error presenting softbuffer buffer: {e}"))?;
        }
        Ok(())
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn occluded(&self, _: bool) {
        // On X11, the buffer is completely cleared when the window is hidden
        // and the buffer age doesn't respect that, so clean the partial rendering cache
        self.renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
    }
}
