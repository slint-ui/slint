// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Delegate the rendering to the [`i_slint_core::software_renderer::SoftwareRenderer`]

use super::WinitCompatibleCanvas;
use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::Rgb8Pixel;
use i_slint_core::lengths::LogicalLength;
pub use i_slint_core::software_renderer::SoftwareRenderer;
use i_slint_core::window::WindowAdapter;
use std::cell::RefCell;
use std::rc::Weak;

impl<const BUFFER_COUNT: usize> super::WinitCompatibleRenderer for SoftwareRenderer<BUFFER_COUNT> {
    type Canvas = SwCanvas;
    const NAME: &'static str = "Software";

    fn new(window_adapter_weak: &Weak<dyn WindowAdapter>) -> Self {
        SoftwareRenderer::new(window_adapter_weak.clone())
    }

    fn create_canvas(
        &self,
        window: &dyn raw_window_handle::HasRawWindowHandle,
        display: &dyn raw_window_handle::HasRawDisplayHandle,
        size: PhysicalWindowSize,
    ) -> Self::Canvas {
        let opengl_context = crate::OpenGLContext::new_context(window, display, size);

        let gl_renderer = unsafe {
            femtovg::renderer::OpenGl::new_from_function(|s| {
                opengl_context.get_proc_address(s) as *const _
            })
            .unwrap()
        };
        let canvas = femtovg::Canvas::new(gl_renderer).unwrap().into();
        SwCanvas { canvas, opengl_context }
    }

    fn release_canvas(&self, _canvas: Self::Canvas) {}

    fn render(&self, canvas: &SwCanvas, size: PhysicalWindowSize) {
        let width = size.width as usize;
        let height = size.height as usize;

        let mut buffer = vec![Rgb8Pixel::default(); width * height];

        if std::env::var_os("SLINT_LINE_BY_LINE").is_none() {
            Self::render(&self, buffer.as_mut_slice(), width);
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
            self.render_by_line(FrameBuffer {
                buffer: &mut buffer,
                line: vec![Default::default(); width],
            });
        }

        let image_ref: imgref::ImgRef<rgb::RGB8> =
            imgref::ImgRef::new(&buffer, width, height).into();

        canvas.opengl_context.make_current();
        {
            let mut canvas = canvas.canvas.borrow_mut();

            canvas.set_size(width as u32, height as u32, 1.0);

            let image_id = canvas.create_image(image_ref, femtovg::ImageFlags::empty()).unwrap();
            let mut path = femtovg::Path::new();
            path.rect(0., 0., image_ref.width() as _, image_ref.height() as _);

            let fill_paint = femtovg::Paint::image(
                image_id,
                0.,
                0.,
                image_ref.width() as _,
                image_ref.height() as _,
                0.0,
                1.0,
            );
            canvas.fill_path(&mut path, fill_paint);
            canvas.flush();
            canvas.delete_image(image_id);
        }

        canvas.opengl_context.swap_buffers();
        canvas.opengl_context.make_not_current();
    }

    fn default_font_size() -> LogicalLength {
        i_slint_core::software_renderer::SoftwareRenderer::<BUFFER_COUNT>::default_font_size()
    }
}

pub(crate) struct SwCanvas {
    canvas: RefCell<femtovg::Canvas<femtovg::renderer::OpenGl>>,
    opengl_context: crate::OpenGLContext,
}

impl WinitCompatibleCanvas for SwCanvas {
    fn component_destroyed(&self, _component: i_slint_core::component::ComponentRef) {}

    fn resize_event(&self, size: PhysicalWindowSize) {
        self.opengl_context.ensure_resized(size)
    }
}
