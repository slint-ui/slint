// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::PlatformError;

#[allow(unused)]
pub trait Presenter {
    fn is_ready_to_present(&self) -> bool;
    fn register_page_flip_handler(
        &self,
        event_loop_handle: crate::calloop_backend::EventLoopHandle,
    ) -> Result<(), PlatformError>;
    // Present updated front-buffer to the screen
    fn present_with_next_frame_callback(
        &self,
        ready_for_next_animation_frame: Box<dyn FnOnce()>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(any(feature = "renderer-skia-opengl", feature = "renderer-femtovg"))]
pub mod gbmdisplay;
#[cfg(any(
    feature = "renderer-skia-opengl",
    feature = "renderer-skia-vulkan",
    feature = "renderer-software"
))]
pub mod swdisplay;
#[cfg(feature = "renderer-skia-vulkan")]
pub mod vulkandisplay;

/// This enum describes the way the output is supposed to be rotated to simulate
/// a screen rotation. This is implemented entirely inside the actual renderer.
#[non_exhaustive]
#[derive(Default, Copy, Clone, Eq, PartialEq, Debug)]
pub enum RenderingRotation {
    /// No rotation
    #[default]
    NoRotation,
    /// Rotate 90° to the right
    Rotate90,
    /// 180° rotation (upside-down)
    Rotate180,
    /// Rotate 90° to the left
    Rotate270,
}

impl TryFrom<&str> for RenderingRotation {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let angle: usize = value.parse().map_err(|_| {
            format!("Invalid value for rotation. Must be unsigned integral, found {value}")
        })?;
        Ok(match angle {
            0 => Self::NoRotation,
            90 => Self::Rotate90,
            180 => Self::Rotate180,
            270 => Self::Rotate270,
            _ => {
                return Err(format!(
                    "Invalid value for rotation. Must be one of 0, 90, 180, or 270"
                ))
            }
        })
    }
}

impl RenderingRotation {
    pub fn screen_size_to_rotated_window_size(&self, screen_size: PhysicalSize) -> PhysicalSize {
        match self {
            RenderingRotation::NoRotation | RenderingRotation::Rotate180 => screen_size,
            RenderingRotation::Rotate90 | RenderingRotation::Rotate270 => {
                PhysicalSize::new(screen_size.height, screen_size.width)
            }
        }
    }

    pub fn degrees(&self) -> f32 {
        match self {
            RenderingRotation::NoRotation => 0.,
            RenderingRotation::Rotate90 => 90.,
            RenderingRotation::Rotate180 => 180.,
            RenderingRotation::Rotate270 => 270.,
        }
    }

    #[allow(unused)]
    pub fn translation_after_rotation(&self, screen_size: PhysicalSize) -> (f32, f32) {
        match self {
            RenderingRotation::NoRotation => (0., 0.),
            RenderingRotation::Rotate90 => (0., -(screen_size.width as f32)),
            RenderingRotation::Rotate180 => {
                (-(screen_size.width as f32), -(screen_size.height as f32))
            }
            RenderingRotation::Rotate270 => (-(screen_size.height as f32), 0.),
        }
    }
}

#[cfg(any(
    feature = "renderer-skia-vulkan",
    feature = "renderer-software",
    feature = "renderer-skia-opengl"
))]
pub(crate) mod timeranimations {
    use i_slint_core::platform::PlatformError;
    use std::cell::Cell;
    use std::rc::{Rc, Weak};

    pub(crate) struct TimerBasedAnimationDriver {
        timer: i_slint_core::timers::Timer,
        next_animation_frame_callback: Cell<Option<Box<dyn FnOnce()>>>,
    }

    impl TimerBasedAnimationDriver {
        pub(crate) fn new() -> Rc<Self> {
            Rc::new_cyclic(|self_weak: &Weak<Self>| {
                let self_weak = self_weak.clone();
                let timer = i_slint_core::timers::Timer::default();
                timer.start(
                    i_slint_core::timers::TimerMode::Repeated,
                    std::time::Duration::from_millis(16),
                    move || {
                        let Some(this) = self_weak.upgrade() else { return };
                        // Stop the timer and let the callback decide if we need to continue. It will set
                        // `needs_redraw` to true of animations should continue, render() will be called,
                        // present_with_next_frame_callback() will be called and then the timer restarted.
                        this.timer.stop();
                        if let Some(next_animation_frame_callback) =
                            this.next_animation_frame_callback.take()
                        {
                            next_animation_frame_callback();
                        }
                    },
                );
                // Activate it only when we present a frame.
                timer.stop();

                Self { timer, next_animation_frame_callback: Default::default() }
            })
        }
    }

    impl crate::display::Presenter for TimerBasedAnimationDriver {
        fn is_ready_to_present(&self) -> bool {
            true
        }

        fn register_page_flip_handler(
            &self,
            _event_loop_handle: crate::calloop_backend::EventLoopHandle,
        ) -> Result<(), PlatformError> {
            Ok(())
        }

        fn present_with_next_frame_callback(
            &self,
            ready_for_next_animation_frame: Box<dyn FnOnce()>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.next_animation_frame_callback.set(Some(ready_for_next_animation_frame));
            self.timer.restart();
            Ok(())
        }
    }
}
