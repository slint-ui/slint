// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! Delegate the rendering to the [`i_slint_core::software_renderer::SoftwareRenderer`]

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::Rgb8Pixel;
use i_slint_core::platform::PlatformError;
use i_slint_core::software_renderer::PremultipliedRgbaColor;
pub use i_slint_core::software_renderer::SoftwareRenderer;
use std::cell::RefCell;

pub struct WinitSoftwareRenderer {
    renderer: SoftwareRenderer,
    _context: softbuffer::Context,
    surface: RefCell<softbuffer::Surface>,
}

impl super::WinitCompatibleRenderer for WinitSoftwareRenderer {
    fn new(
        window_builder: winit::window::WindowBuilder,
    ) -> Result<(Self, winit::window::Window), PlatformError> {
        let winit_window = crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).map_err(|winit_os_error| {
                format!("Error creating native window for software rendering: {}", winit_os_error)
            })
        })?;

        let context = unsafe {
            softbuffer::Context::new(&winit_window)
                .map_err(|e| format!("Error creating softbuffer context: {e}"))?
        };

        let surface = unsafe { softbuffer::Surface::new(&context, &winit_window) }.map_err(
            |softbuffer_error| format!("Error creating softbuffer surface: {}", softbuffer_error),
        )?;

        Ok((
            Self {
                renderer: SoftwareRenderer::new_without_window(
                    i_slint_core::software_renderer::RepaintBufferType::NewBuffer,
                ),
                _context: context,
                surface: RefCell::new(surface),
            },
            winit_window,
        ))
    }

    fn render(&self, window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        let size = window.size();

        let width = size.width.try_into().map_err(|_| {
            format!(
                "Attempting to resize softbuffer window surface with an invalid width: {}",
                size.width
            )
        })?;
        let height = size.height.try_into().map_err(|_| {
            format!(
                "Attempting to resize softbuffer window surface with an invalid height: {}",
                size.height
            )
        })?;

        self.renderer.set_window(window);

        let mut surface = self.surface.borrow_mut();

        surface
            .resize(width, height)
            .map_err(|e| format!("Error resizing softbuffer surface: {e}"))?;

        let mut target_buffer = surface
            .buffer_mut()
            .map_err(|e| format!("Error retrieving softbuffer rendering buffer: {e}"))?;

        if std::env::var_os("SLINT_LINE_BY_LINE").is_none() {
            let mut buffer = vec![
                PremultipliedRgbaColor::default();
                width.get() as usize * height.get() as usize
            ];
            self.renderer.render(buffer.as_mut_slice(), width.get() as usize);

            for i in 0..target_buffer.len() {
                let pixel = buffer[i];
                target_buffer[i] = (pixel.alpha as u32) << 24
                    | ((pixel.red as u32) << 16)
                    | ((pixel.green as u32) << 8)
                    | (pixel.blue as u32);
            }
        } else {
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
            });
        };

        target_buffer.present().map_err(|e| format!("Error presenting softbuffer buffer: {e}"))?;

        Ok(())
    }

    fn resize_event(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        let width = size.width.try_into().map_err(|_| {
            format!(
                "Attempting to resize softbuffer window surface with an invalid width: {}",
                size.width
            )
        })?;
        let height = size.height.try_into().map_err(|_| {
            format!(
                "Attempting to resize softbuffer window surface with an invalid height: {}",
                size.height
            )
        })?;

        self.surface
            .borrow_mut()
            .resize(width, height)
            .map_err(|e| format!("Error resizing softbuffer surface: {e}").into())
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }
}
