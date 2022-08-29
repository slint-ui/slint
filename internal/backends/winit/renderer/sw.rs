// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Delegate the rendeing to the [`i_slint_core::swrenderer::SoftwareRenderer`]

use super::WinitCompatibleCanvas;
use i_slint_core::graphics::Rgb8Pixel;
use i_slint_core::lengths::PhysicalLength;
pub use i_slint_core::swrenderer::SoftwareRenderer;
use i_slint_core::window::PlatformWindow;
use std::cell::RefCell;
use std::rc::Weak;

impl super::WinitCompatibleRenderer for SoftwareRenderer {
    type Canvas = SwCanvas;

    fn new(platform_window_weak: &Weak<dyn PlatformWindow>) -> Self {
        SoftwareRenderer::new(
            i_slint_core::swrenderer::DirtyTracking::None,
            platform_window_weak.clone(),
        )
    }

    fn create_canvas(&self, window_builder: winit::window::WindowBuilder) -> Self::Canvas {
        let opengl_context = crate::OpenGLContext::new_context(window_builder);

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

    fn render(&self, canvas: &SwCanvas, _: &dyn PlatformWindow) {
        let size = canvas.opengl_context.window().inner_size();
        let width = size.width as usize;
        let height = size.height as usize;

        let mut buffer = vec![Rgb8Pixel::default(); width * height];

        if std::env::var_os("SLINT_LINE_BY_LINE").is_none() {
            Self::render(&self, buffer.as_mut_slice(), PhysicalLength::new(width as _));
        } else {
            struct FrameBuffer<'a> {
                buffer: &'a mut [Rgb8Pixel],
                line: Vec<i_slint_core::swrenderer::Rgb565Pixel>,
            }
            impl<'a> i_slint_core::swrenderer::LineBufferProvider for FrameBuffer<'a> {
                type TargetPixel = i_slint_core::swrenderer::Rgb565Pixel;
                fn process_line(
                    &mut self,
                    line: PhysicalLength,
                    range: core::ops::Range<PhysicalLength>,
                    render_fn: impl FnOnce(&mut [Self::TargetPixel]),
                ) {
                    let len = (range.end.get() - range.start.get()) as usize;
                    let line_begin = line.get() as usize * self.line.len();
                    let sub = &mut self.line[..len];
                    render_fn(sub);
                    for (dst, src) in self.buffer[line_begin + (range.start.get() as usize)
                        ..line_begin + (range.end.get() as usize)]
                        .iter_mut()
                        .zip(sub)
                    {
                        dst.r = src.red();
                        dst.g = src.green();
                        dst.b = src.blue();
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
}

pub(crate) struct SwCanvas {
    canvas: RefCell<femtovg::Canvas<femtovg::renderer::OpenGl>>,
    opengl_context: crate::OpenGLContext,
}

impl WinitCompatibleCanvas for SwCanvas {
    fn component_destroyed(&self, _component: i_slint_core::component::ComponentRef) {}

    fn with_window_handle<T>(&self, callback: impl FnOnce(&winit::window::Window) -> T) -> T {
        callback(&*self.opengl_context.window())
    }

    fn resize_event(&self) {
        self.opengl_context.ensure_resized()
    }
}
