// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the window adapter implementation to communicate between Slint and Vulkan + libinput

use std::cell::{Cell, RefCell};
use std::pin::Pin;
use std::rc::Rc;

use i_slint_core::Property;
use i_slint_core::api::{LogicalPosition, PhysicalSize as PhysicalWindowSize};
use i_slint_core::cursor::MouseCursorInner;
use i_slint_core::graphics::{Image, euclid};
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::items::BuiltInMouseCursor;
use i_slint_core::lengths::{LogicalRect, LogicalVector};
use i_slint_core::platform::WindowEvent;
use i_slint_core::renderer::DrawOutcome;
use i_slint_core::slice::Slice;
use i_slint_core::{platform::PlatformError, window::WindowAdapter, window::WindowAdapterInternal};

use crate::display::RenderingRotation;

#[cfg(all(test, enable_skia))]
#[path = "tests/cursor/mod.rs"]
mod tests;

pub trait FullscreenRenderer {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer;
    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<DrawOutcome, PlatformError>;
    fn size(&self) -> PhysicalWindowSize;
}

pub struct FullscreenWindowAdapter {
    window: i_slint_core::api::Window,
    renderer: Box<dyn FullscreenRenderer>,
    redraw_requested: Cell<bool>,
    rotation: RenderingRotation,
    loop_signal: RefCell<Option<calloop::LoopSignal>>,
    mouse_cursor: RefCell<MouseCursorInner>,
    last_cursor_rect: Cell<Option<LogicalRect>>,
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
        self.redraw_requested.set(true);
        if let Some(signal) = self.loop_signal.borrow().as_ref() {
            signal.wakeup();
        }
    }

    fn set_visible(&self, visible: bool) -> Result<(), PlatformError> {
        if visible
            && let Some(scale_factor) =
                std::env::var("SLINT_SCALE_FACTOR").ok().and_then(|sf| sf.parse().ok())
        {
            self.window
                .dispatch_event_with_result(WindowEvent::ScaleFactorChanged { scale_factor })?;
        }
        Ok(())
    }

    fn internal(&self, _: i_slint_core::InternalToken) -> Option<&dyn WindowAdapterInternal> {
        Some(self)
    }
}

impl WindowAdapterInternal for FullscreenWindowAdapter {
    fn set_mouse_cursor(&self, cursor: MouseCursorInner) {
        if *self.mouse_cursor.borrow() != cursor {
            *self.mouse_cursor.borrow_mut() = cursor;
            self.request_redraw();
        }
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
            loop_signal: RefCell::new(None),
            mouse_cursor: RefCell::new(MouseCursorInner::default()),
            last_cursor_rect: Cell::new(None),
        }))
    }

    pub fn set_loop_signal(&self, signal: calloop::LoopSignal) {
        *self.loop_signal.borrow_mut() = Some(signal);
    }

    pub fn render_if_needed(
        self: Rc<Self>,
        mouse_position: Pin<&Property<Option<LogicalPosition>>>,
    ) -> Result<(), PlatformError> {
        if self.redraw_requested.replace(false) {
            let cursor = cursor_image_and_rect(&self.mouse_cursor.borrow(), mouse_position.get());
            let cursor_rect = cursor.as_ref().map(|(_, rect)| *rect);
            // Damage must reach the renderer before it calculates this frame's clip.
            // Repaint the old position as well, including when the cursor is hidden.
            for rect in self.last_cursor_rect.get().into_iter().chain(cursor_rect) {
                self.renderer.as_core_renderer().mark_dirty_region(rect.into());
            }
            let outcome = self.renderer.render_and_present(self.rotation, &|item_renderer| {
                // Keep pointer movement in the window's draw dependency tracker.
                let _ = mouse_position.get();
                if let Some((image, rect)) = &cursor {
                    item_renderer.save_state();
                    item_renderer.translate(rect.origin.to_vector());
                    item_renderer.draw_image_direct(image.clone());
                    item_renderer.restore_state();
                }
            })?;
            if matches!(outcome, DrawOutcome::Success) {
                self.last_cursor_rect.set(cursor_rect);
            } else {
                self.redraw_requested.set(true);
            }
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

fn cursor_image_and_rect(
    cursor: &MouseCursorInner,
    position: Option<LogicalPosition>,
) -> Option<(Image, LogicalRect)> {
    let position = position?;
    let (image, hotspot) = match cursor {
        MouseCursorInner::CustomMouseCursor { image, hotspot_x, hotspot_y } => {
            let size = image.size();
            let hotspot = LogicalVector::new(
                (*hotspot_x).clamp(0, size.width.saturating_sub(1) as i32) as f32,
                (*hotspot_y).clamp(0, size.height.saturating_sub(1) as i32) as f32,
            );
            (image.clone(), hotspot)
        }
        MouseCursorInner::BuiltIn(BuiltInMouseCursor::None) => return None,
        _ => (mouse_cursor_image(), LogicalVector::default()),
    };
    let origin = i_slint_core::lengths::logical_point_from_api(position) - hotspot;
    let rect = LogicalRect::new(origin, euclid::Size2D::from_untyped(image.size().cast()));
    Some((image, rect))
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
        cached_image => cached_image.clone().into(),
    }
}
