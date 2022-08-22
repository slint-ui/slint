// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Delegate the rendeing to the [`i_slint_core::swrenderer::SoftwareRenderer`]

use super::WinitCompatibleCanvas;
use i_slint_core::graphics::Rgb8Pixel;
use i_slint_core::lengths::PhysicalLength;
pub use i_slint_core::swrenderer::SoftwareRenderer;
use i_slint_core::window::{PlatformWindow, WindowHandleAccess};
use std::cell::RefCell;
use std::rc::Weak;

impl super::WinitCompatibleRenderer for SoftwareRenderer {
    type Canvas = SwCanvas;

    fn new(_platform_window_weak: &Weak<dyn PlatformWindow>) -> Self {
        SoftwareRenderer::new(i_slint_core::swrenderer::DirtyTracking::None)
    }

    fn create_canvas(&self, window_builder: winit::window::WindowBuilder) -> Self::Canvas {
        let opengl_context = crate::OpenGLContext::new_context(window_builder);

        let gl_renderer =
            femtovg::renderer::OpenGl::new_from_glutin_context(&opengl_context.glutin_context())
                .unwrap();
        let canvas = femtovg::Canvas::new(gl_renderer).unwrap().into();
        SwCanvas { canvas, opengl_context }
    }

    fn release_canvas(&self, _canvas: Self::Canvas) {}

    fn render(&self, canvas: &SwCanvas, platform_window: &dyn PlatformWindow) {
        let size = canvas.opengl_context.window().inner_size();
        let width = size.width as usize;
        let height = size.height as usize;

        let window = platform_window.window().window_handle();

        let mut buffer = vec![Rgb8Pixel::default(); width * height];

        window.draw_contents(|_component| {
            Self::render(
                &self,
                platform_window.window(),
                buffer.as_mut_slice(),
                PhysicalLength::new(width as _),
            );
        });

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
