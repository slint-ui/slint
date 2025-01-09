// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the window adapter implementation to communicate between Slint and Vulkan + libinput

use std::cell::Cell;
use std::pin::Pin;
use std::rc::Rc;

use i_slint_core::api::{LogicalPosition, PhysicalSize as PhysicalWindowSize};
use i_slint_core::graphics::{euclid, Image};
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::lengths::LogicalRect;
use i_slint_core::platform::WindowEvent;
use i_slint_core::slice::Slice;
use i_slint_core::Property;
use i_slint_core::{platform::PlatformError, window::WindowAdapter};

use crate::display::RenderingRotation;

pub trait FullscreenRenderer {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer;
    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<(), PlatformError>;
    fn size(&self) -> PhysicalWindowSize;
}

pub struct FullscreenWindowAdapter {
    window: i_slint_core::api::Window,
    renderer: Box<dyn FullscreenRenderer>,
    redraw_requested: Cell<bool>,
    rotation: RenderingRotation,
}

impl WindowAdapter for FullscreenWindowAdapter {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.rotation.screen_size_to_rotated_window_size(self.renderer.size())
    }

    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        self.renderer.as_core_renderer()
    }

    fn request_redraw(&self) {
        self.redraw_requested.set(true)
    }

    fn set_visible(&self, visible: bool) -> Result<(), PlatformError> {
        if visible {
            if let Some(scale_factor) =
                std::env::var("SLINT_SCALE_FACTOR").ok().and_then(|sf| sf.parse().ok())
            {
                self.window.try_dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor })?;
            }
        }
        Ok(())
    }
}

impl FullscreenWindowAdapter {
    pub fn new(
        renderer: Box<dyn FullscreenRenderer>,
        rotation: RenderingRotation,
    ) -> Result<Rc<Self>, PlatformError> {
        let size = renderer.size();
        let rotation_degrees = rotation.degrees();
        eprintln!(
            "Rendering at {}x{}{}",
            size.width,
            size.height,
            if rotation_degrees != 0. {
                format!(" with {} rotation_degrees rotation", rotation_degrees)
            } else {
                String::new()
            }
        );
        Ok(Rc::<FullscreenWindowAdapter>::new_cyclic(|self_weak| FullscreenWindowAdapter {
            window: i_slint_core::api::Window::new(self_weak.clone()),
            renderer,
            redraw_requested: Cell::new(true),
            rotation,
        }))
    }

    pub fn render_if_needed(
        self: Rc<Self>,
        mouse_position: Pin<&Property<Option<LogicalPosition>>>,
    ) -> Result<(), PlatformError> {
        if self.redraw_requested.replace(false) {
            self.renderer.render_and_present(self.rotation, &|item_renderer| {
                if let Some(mouse_position) = mouse_position.get() {
                    let cursor_image = mouse_cursor_image();
                    item_renderer.save_state();
                    item_renderer.translate(
                        i_slint_core::lengths::logical_point_from_api(mouse_position).to_vector(),
                    );
                    item_renderer.draw_image_direct(mouse_cursor_image());
                    item_renderer.restore_state();
                    let cursor_rect = LogicalRect::new(
                        euclid::point2(mouse_position.x, mouse_position.y),
                        euclid::Size2D::from_untyped(cursor_image.size().cast()),
                    );
                    self.renderer.as_core_renderer().mark_dirty_region(cursor_rect.into());
                }
            })?;
            // Check once after rendering if we have running animations and
            // remember that to trigger a redraw after the frame is on the screen.
            // Timers might have been updated if the event loop is woken up
            // due to other reasons, which would also reset has_active_animations.
            if self.window.has_active_animations() {
                let self_weak = Rc::downgrade(&self);
                i_slint_core::timers::Timer::single_shot(
                    std::time::Duration::default(),
                    move || {
                        let Some(this) = self_weak.upgrade() else {
                            return;
                        };
                        this.request_redraw();
                    },
                )
            }
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
            let pixels = svg.render(None).unwrap();
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
