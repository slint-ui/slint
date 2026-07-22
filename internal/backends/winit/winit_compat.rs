// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Polyfill for `winit::window::Window::surface_size()`, which becomes a built-in method in
//! winit 0.31.

pub(crate) trait WindowSurfaceSizeExt {
    fn surface_size(&self) -> winit::dpi::PhysicalSize<u32>;
}

impl WindowSurfaceSizeExt for winit::window::Window {
    /// The physical size of the rendering surface backing a winit window.
    fn surface_size(&self) -> winit::dpi::PhysicalSize<u32> {
        // winit 0.30's iOS `inner_size()` returns the safe-area frame but the `CAMetalLayer` fills the whole
        // window, so the render surface must match `outer_size()`. That's also what `WindowEvent::Resized`
        // delivers.
        if cfg!(target_os = "ios") { self.outer_size() } else { self.inner_size() }
    }
}
