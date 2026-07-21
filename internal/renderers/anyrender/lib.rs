// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Slint renderer scaffolding generic over an [`anyrender`] backend.
//!
//! This crate provides:
//! - [`SlintWindowRenderer`]: a small extension on top of
//!   [`anyrender::WindowRenderer`] adding the fallible operations Slint
//!   needs (a per-frame render with a base color and a `Result`-returning
//!   draw closure, and a fallible resize).
//!
//! Concrete backends (vello over wgpu, vello_cpu over softbuffer, …) live
//! in their own crates and only need to implement `SlintWindowRenderer`.

#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::platform::PlatformError;

/// Slint-side extension to [`anyrender::WindowRenderer`].
///
/// Adds the fallible operations Slint needs that do not fit anyrender's
/// own `WindowRenderer` signature — namely a per-frame render with a
/// caller-supplied base color and a `Result`-returning closure, and a
/// fallible resize.
pub trait SlintWindowRenderer: anyrender::WindowRenderer {
    fn slint_render<F>(
        &mut self,
        surface_size: i_slint_core::api::PhysicalSize,
        base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        draw: F,
    ) -> Result<(), PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>;

    fn slint_set_size(&mut self, width: u32, height: u32) -> Result<(), PlatformError>;

    /// Render `draw` into a CPU-readable RGBA8 buffer instead of presenting
    /// to a surface. Used by [`Window::take_snapshot`](i_slint_core::api::Window::take_snapshot).
    ///
    /// The default impl returns an error; backends override.
    fn slint_take_snapshot<F>(
        &mut self,
        _surface_size: i_slint_core::api::PhysicalSize,
        _base_color: peniko::color::AlphaColor<peniko::color::Srgb>,
        _draw: F,
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError>
    where
        F: FnOnce(&mut Self::ScenePainter<'_>) -> Result<(), PlatformError>,
    {
        Err("take_snapshot is not implemented for this anyrender backend".into())
    }
}
