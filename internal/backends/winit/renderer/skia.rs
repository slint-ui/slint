// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::rc::Rc;

use crate::winitwindowadapter::physical_size_to_slint;
use i_slint_core::platform::PlatformError;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

pub struct WinitSkiaRenderer {
    renderer: i_slint_renderer_skia::SkiaRenderer,
    winit_window: Rc<winit::window::Window>,
}

impl WinitSkiaRenderer {
    pub fn new(
        window_builder: winit::window::WindowBuilder,
    ) -> Result<(Box<dyn super::WinitCompatibleRenderer>, Rc<winit::window::Window>), PlatformError>
    {
        let winit_window = Rc::new(crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).map_err(|winit_os_error| {
                format!("Error creating native window for Skia rendering: {}", winit_os_error)
                    .into()
            })
        })?);

        let renderer = i_slint_renderer_skia::SkiaRenderer::default();

        Ok((Box::new(Self { renderer, winit_window: winit_window.clone() }), winit_window))
    }

    #[cfg(not(target_os = "android"))]
    pub fn new_software(
        window_builder: winit::window::WindowBuilder,
    ) -> Result<(Box<dyn super::WinitCompatibleRenderer>, Rc<winit::window::Window>), PlatformError>
    {
        let winit_window = Rc::new(crate::event_loop::with_window_target(|event_loop| {
            window_builder.build(event_loop.event_loop_target()).map_err(|winit_os_error| {
                format!("Error creating native window for Skia rendering: {}", winit_os_error)
                    .into()
            })
        })?);

        let renderer = i_slint_renderer_skia::SkiaRenderer::default_software();

        Ok((Box::new(Self { renderer, winit_window: winit_window.clone() }), winit_window))
    }
}

impl super::WinitCompatibleRenderer for WinitSkiaRenderer {
    fn render(&self, _window: &i_slint_core::api::Window) -> Result<(), PlatformError> {
        let winit_window = self.winit_window.clone();
        self.renderer.render(Some(Box::new(move || {
            winit_window.pre_present_notify();
        })))
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn resumed(&self, winit_window: &winit::window::Window) -> Result<(), PlatformError> {
        let size = winit_window.inner_size();

        // Safety: This is safe because the handle remains valid; the next rwh release provides `new()` without unsafe.
        let active_handle = unsafe { raw_window_handle::ActiveHandle::new_unchecked() };

        // Safety: API wise we can't guarantee that the window/display handles remain valid, so we
        // use unsafe here. However the winit window adapter keeps the winit window alive as long as
        // the renderer.
        // TODO: remove once winit implements HasWindowHandle/HasDisplayHandle
        let (window_handle, display_handle) = unsafe {
            (
                raw_window_handle::WindowHandle::borrow_raw(
                    winit_window.raw_window_handle(),
                    active_handle,
                ),
                raw_window_handle::DisplayHandle::borrow_raw(winit_window.raw_display_handle()),
            )
        };

        self.renderer.set_window_handle(
            window_handle,
            display_handle,
            physical_size_to_slint(&size),
        )
    }
}
