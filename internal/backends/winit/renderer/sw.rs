// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Delegate the rendering to the [`i_slint_core::software_renderer::SoftwareRenderer`]

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::Rgb8Pixel;
use i_slint_core::platform::PlatformError;
pub use i_slint_core::software_renderer::SoftwareRenderer;
use i_slint_core::window::WindowAdapter;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub struct WinitSoftwareRenderer {
    renderer: SoftwareRenderer,
    canvas: RefCell<Option<softbuffer::GraphicsContext>>,
}

impl super::WinitCompatibleRenderer for WinitSoftwareRenderer {
    const NAME: &'static str = "Software";

    fn new(window_adapter_weak: &Weak<dyn WindowAdapter>) -> Self {
        Self {
            renderer: SoftwareRenderer::new(
                i_slint_core::software_renderer::RepaintBufferType::NewBuffer,
                window_adapter_weak.clone(),
            ),
            canvas: Default::default(),
        }
    }

    fn show(
        &self,
        window_builder: winit::window::WindowBuilder,
    ) -> Result<Rc<winit::window::Window>, PlatformError> {
        let window = crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).map_err(|winit_os_error| {
                format!("Error creating native window for software rendering: {}", winit_os_error)
            })
        })?;
        let window = Rc::new(window);

        *self.canvas.borrow_mut() = Some(unsafe {
            softbuffer::GraphicsContext::new(window.as_ref(), window.as_ref()).map_err(
                |softbuffer_error| {
                    format!("Error creating softbuffer graphics context: {}", softbuffer_error)
                },
            )?
        });

        Ok(window)
    }

    fn hide(&self) -> Result<(), PlatformError> {
        self.canvas.borrow_mut().take();
        Ok(())
    }

    fn render(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        let mut canvas = if self.canvas.borrow().is_some() {
            std::cell::RefMut::map(self.canvas.borrow_mut(), |canvas_opt| {
                canvas_opt.as_mut().unwrap()
            })
        } else {
            return Ok(());
        };

        let width = size.width as usize;
        let height = size.height as usize;

        let mut buffer = vec![Rgb8Pixel::default(); width * height];

        if std::env::var_os("SLINT_LINE_BY_LINE").is_none() {
            self.renderer.render(buffer.as_mut_slice(), width);
        } else {
            struct FrameBuffer<'a> {
                buffer: &'a mut [Rgb8Pixel],
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
                        *dst = (*src).into();
                    }
                }
            }
            self.renderer.render_by_line(FrameBuffer {
                buffer: &mut buffer,
                line: vec![Default::default(); width],
            });
        }

        let mut softbuffer_buffer = Vec::with_capacity(width * height * 4);
        softbuffer_buffer.extend(
            buffer
                .into_iter()
                .map(|pixel| ((pixel.r as u32) << 16) | ((pixel.g as u32) << 8) | (pixel.b as u32)),
        );

        canvas.set_buffer(&softbuffer_buffer, width as u16, height as u16);
        Ok(())
    }

    fn resize_event(&self, _size: PhysicalWindowSize) -> Result<(), PlatformError> {
        Ok(())
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }
}
