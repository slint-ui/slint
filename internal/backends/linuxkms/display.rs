// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize;

#[allow(unused)]
pub trait Presenter {
    // Present updated front-buffer to the screen
    fn present(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
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
pub(crate) mod noop_presenter {
    use std::rc::Rc;

    // Used when the underlying renderer/display takes care of the presentation to the display
    // and (hopefully) implements vsync.
    pub(crate) struct NoopPresenter {}

    impl NoopPresenter {
        pub(crate) fn new() -> Rc<Self> {
            Rc::new(Self {})
        }
    }

    impl crate::display::Presenter for NoopPresenter {
        fn present(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
    }
}
