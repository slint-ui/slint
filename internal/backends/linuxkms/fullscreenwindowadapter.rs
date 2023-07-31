// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! This module contains the window adapter implementation to communicate between Slint and Vulkan + libinput

use std::cell::Cell;
use std::pin::Pin;
use std::rc::Rc;

use i_slint_core::api::{LogicalPosition, PhysicalSize as PhysicalWindowSize};
use i_slint_core::graphics::euclid;
use i_slint_core::graphics::Image;
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::platform::WindowEvent;
use i_slint_core::slice::Slice;
use i_slint_core::Property;
use i_slint_core::{platform::PlatformError, window::WindowAdapter};

pub trait FullscreenRenderer {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer;
    fn render_and_present(
        &self,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<(), PlatformError>;
    fn size(&self) -> PhysicalWindowSize;
}

pub struct FullscreenWindowAdapter {
    window: i_slint_core::api::Window,
    renderer: Box<dyn FullscreenRenderer>,
    needs_redraw: Cell<bool>,
}

impl WindowAdapter for FullscreenWindowAdapter {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.renderer.size()
    }

    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        self.renderer.as_core_renderer()
    }

    fn request_redraw(&self) {
        self.needs_redraw.set(true)
    }

    fn set_visible(&self, visible: bool) -> Result<(), PlatformError> {
        if visible {
            let scale_factor = if let Some(scale_factor) =
                std::env::var("SLINT_SCALE_FACTOR").ok().and_then(|sf| sf.parse().ok())
            {
                self.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor });
                scale_factor
            } else {
                1.0
            };
            let size = self.size().to_logical(scale_factor);
            self.window.dispatch_event(WindowEvent::Resized { size });
        }
        Ok(())
    }
}

impl FullscreenWindowAdapter {
    pub fn new(renderer: Box<dyn FullscreenRenderer>) -> Result<Rc<Self>, PlatformError> {
        Ok(Rc::<FullscreenWindowAdapter>::new_cyclic(|self_weak| FullscreenWindowAdapter {
            window: i_slint_core::api::Window::new(self_weak.clone()),
            renderer,
            needs_redraw: Cell::new(true),
        }))
    }

    pub fn render_if_needed(
        &self,
        mouse_position: Pin<&Property<Option<LogicalPosition>>>,
    ) -> Result<(), PlatformError> {
        if self.needs_redraw.replace(false) {
            self.renderer.render_and_present(&|item_renderer| {
                if let Some(mouse_position) = mouse_position.get() {
                    item_renderer.save_state();
                    item_renderer.translate(
                        i_slint_core::lengths::logical_point_from_api(mouse_position).to_vector(),
                    );
                    item_renderer.draw_image_direct(mouse_cursor_image());
                    item_renderer.restore_state();
                }
            })?;
        }
        Ok(())
    }
}

fn mouse_cursor_image() -> Image {
    let mouse_pointer_svg = i_slint_core::graphics::load_image_from_embedded_data(
        Slice::from_slice(include_bytes!("mouse-pointer.svg")),
        Slice::from_slice(b"svg"),
    );
    let mouse_pointer_inner: &i_slint_core::graphics::ImageInner = (&mouse_pointer_svg).into();
    match mouse_pointer_inner {
        i_slint_core::ImageInner::Svg(svg) => {
            let pixels = svg.render(euclid::Size2D::from_untyped(svg.size())).unwrap();
            let cache_key = svg.cache_key();
            let mouse_pointer_pixel_image = i_slint_core::graphics::ImageInner::EmbeddedImage {
                cache_key: cache_key.clone(),
                buffer: pixels,
            };
            i_slint_core::graphics::cache::replace_cached_image(
                cache_key,
                mouse_pointer_pixel_image.clone(),
            );

            mouse_pointer_pixel_image.into()
        }
        cached_image @ _ => cached_image.clone().into(),
    }
}
