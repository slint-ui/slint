// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the window adapter implementation to communicate between Slint and Vulkan + libinput

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::cursor::{MouseCursorHandling, MouseCursorInner};
use i_slint_core::graphics::Image;
use i_slint_core::platform::WindowEvent;
use i_slint_core::renderer::DrawOutcome;
use i_slint_core::slice::Slice;
use i_slint_core::{platform::PlatformError, window::WindowAdapter, window::WindowAdapterInternal};

use crate::display::RenderingRotation;

#[cfg(all(test, enable_skia))]
mod tests;

pub trait FullscreenRenderer {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer;
    fn render_and_present(&self, rotation: RenderingRotation)
    -> Result<DrawOutcome, PlatformError>;
    /// Puts a frame that [`Self::render_and_present`] left pending on the screen.
    /// The event loop calls this when no frame follows right away, and when it ends.
    fn flush_pending_frame(&self) -> Result<(), PlatformError> {
        Ok(())
    }
    fn size(&self) -> PhysicalWindowSize;
}

pub struct FullscreenWindowAdapter {
    window: i_slint_core::api::Window,
    renderer: Box<dyn FullscreenRenderer>,
    redraw_requested: Cell<bool>,
    rotation: RenderingRotation,
    loop_signal: RefCell<Option<calloop::LoopSignal>>,
    mouse_cursor_image: OnceCell<Image>,
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
    fn set_mouse_cursor(&self, cursor: MouseCursorInner) -> MouseCursorHandling {
        // There is no compositor to draw a cursor on top of the frame.
        MouseCursorHandling::render_with_default_image(&cursor, || {
            self.mouse_cursor_image.get_or_init(mouse_cursor_image).clone()
        })
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
            mouse_cursor_image: OnceCell::new(),
        }))
    }

    pub fn set_loop_signal(&self, signal: calloop::LoopSignal) {
        *self.loop_signal.borrow_mut() = Some(signal);
    }

    pub fn render_if_needed(self: Rc<Self>) -> Result<(), PlatformError> {
        if self.redraw_requested.replace(false) {
            let outcome = self.renderer.render_and_present(self.rotation)?;
            if !matches!(outcome, DrawOutcome::Success) {
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

    pub fn flush_unless_frame_follows(&self) -> Result<(), PlatformError> {
        if self.redraw_requested.get() || self.window.has_active_animations() {
            return Ok(());
        }
        self.renderer.flush_pending_frame()
    }

    pub fn flush_pending_frame(&self) -> Result<(), PlatformError> {
        self.renderer.flush_pending_frame()
    }
}

/// The arrow drawn for the built-in cursors, rasterized once instead of in every frame.
fn mouse_cursor_image() -> Image {
    let svg = i_slint_core::graphics::load_image_from_embedded_data(
        Slice::from_slice(include_bytes!("mouse-pointer.svg")),
        Slice::from_slice(b"svg"),
    );
    let inner: &i_slint_core::ImageInner = (&svg).into();
    match inner {
        i_slint_core::ImageInner::Svg(inner) => i_slint_core::ImageInner::EmbeddedImage {
            cache_key: inner.cache_key().clone(),
            buffer: inner.render(None).unwrap(),
        }
        .into(),
        _ => svg.clone(),
    }
}
