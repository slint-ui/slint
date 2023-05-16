// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

//! Delegate the rendering to the [`i_slint_core::software_renderer::SoftwareRenderer`]

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::Rgb8Pixel;
use i_slint_core::platform::PlatformError;
use i_slint_core::software_renderer::PremultipliedRgbaColor;
pub use i_slint_core::software_renderer::SoftwareRenderer;
use i_slint_core::window::WindowAdapter;
use std::cell::RefCell;
use std::rc::Weak;

pub struct WinitSoftwareRenderer {
    renderer: SoftwareRenderer,
    canvas: RefCell<softbuffer::GraphicsContext>,
}

impl super::WinitCompatibleRenderer for WinitSoftwareRenderer {
    fn new(
        window_adapter_weak: &Weak<dyn WindowAdapter>,
        window_builder: winit::window::WindowBuilder,
    ) -> Result<(Self, winit::window::Window), PlatformError> {
        let winit_window = crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).map_err(|winit_os_error| {
                format!("Error creating native window for software rendering: {}", winit_os_error)
            })
        })?;

        let canvas = unsafe { softbuffer::GraphicsContext::new(&winit_window, &winit_window) }
            .map_err(|softbuffer_error| {
                format!("Error creating softbuffer graphics context: {}", softbuffer_error)
            })?;

        Ok((
            Self {
                renderer: SoftwareRenderer::new(
                    i_slint_core::software_renderer::RepaintBufferType::NewBuffer,
                    window_adapter_weak.clone(),
                ),
                canvas: RefCell::new(canvas),
            },
            winit_window,
        ))
    }

    fn show(&self) -> Result<(), PlatformError> {
        Ok(())
    }

    fn hide(&self) -> Result<(), PlatformError> {
        Ok(())
    }

    fn render(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        let width = size.width as usize;
        let height = size.height as usize;

        let softbuffer_buffer = if std::env::var_os("SLINT_LINE_BY_LINE").is_none() {
            let mut buffer = vec![PremultipliedRgbaColor::default(); width * height];
            self.renderer.render(buffer.as_mut_slice(), width);
            buffer
                .into_iter()
                .map(|pixel| {
                    (pixel.alpha as u32) << 24
                        | ((pixel.red as u32) << 16)
                        | ((pixel.green as u32) << 8)
                        | (pixel.blue as u32)
                })
                .collect::<Vec<_>>()
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
            let mut softbuffer_buffer = vec![0u32; width * height];
            self.renderer.render_by_line(FrameBuffer {
                buffer: &mut softbuffer_buffer,
                line: vec![Default::default(); width],
            });
            softbuffer_buffer
        };
        self.canvas.borrow_mut().set_buffer(&softbuffer_buffer, width as u16, height as u16);

        Ok(())
    }

    fn resize_event(&self, _size: PhysicalWindowSize) -> Result<(), PlatformError> {
        Ok(())
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }
}
