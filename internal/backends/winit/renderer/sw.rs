// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Delegate the rendering to the [`i_slint_core::software_renderer::SoftwareRenderer`]

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::graphics::Rgb8Pixel;
use i_slint_core::lengths::LogicalLength;
pub use i_slint_core::software_renderer::SoftwareRenderer;
use i_slint_core::window::WindowAdapter;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub struct WinitSoftwareRenderer<const MAX_BUFFER_AGE: usize> {
    renderer: SoftwareRenderer<MAX_BUFFER_AGE>,
    canvas: RefCell<Option<SwCanvas>>,
}

impl<const MAX_BUFFER_AGE: usize> super::WinitCompatibleRenderer
    for WinitSoftwareRenderer<MAX_BUFFER_AGE>
{
    const NAME: &'static str = "Software";

    fn new(window_adapter_weak: &Weak<dyn WindowAdapter>) -> Self {
        Self {
            renderer: SoftwareRenderer::new(window_adapter_weak.clone()),
            canvas: Default::default(),
        }
    }

    fn show(&self, window: &Rc<winit::window::Window>) {
        let size: winit::dpi::PhysicalSize<u32> = window.inner_size();
        let opengl_context = crate::OpenGLContext::new_context(
            window,
            window,
            PhysicalWindowSize::new(size.width, size.height),
        );

        let gl_renderer = unsafe {
            femtovg::renderer::OpenGl::new_from_function(|s| {
                opengl_context.get_proc_address(s) as *const _
            })
            .unwrap()
        };
        let canvas = femtovg::Canvas::new(gl_renderer).unwrap().into();
        *self.canvas.borrow_mut() = Some(SwCanvas { canvas, opengl_context })
    }

    fn hide(&self) {
        self.canvas.borrow_mut().take();
    }

    fn render(&self, size: PhysicalWindowSize) {
        let canvas = if self.canvas.borrow().is_some() {
            std::cell::Ref::map(self.canvas.borrow(), |canvas_opt| canvas_opt.as_ref().unwrap())
        } else {
            return;
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

    fn component_destroyed(&self, _component: i_slint_core::component::ComponentRef) {}

    fn resize_event(&self, size: PhysicalWindowSize) {
        let canvas = if self.canvas.borrow().is_some() {
            std::cell::Ref::map(self.canvas.borrow(), |canvas_opt| canvas_opt.as_ref().unwrap())
        } else {
            return;
        };

        canvas.opengl_context.ensure_resized(size)
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn default_font_size(&self) -> LogicalLength {
        self.renderer.default_font_size()
    }
}

struct SwCanvas {
    canvas: RefCell<femtovg::Canvas<femtovg::renderer::OpenGl>>,
    opengl_context: crate::OpenGLContext,
}
