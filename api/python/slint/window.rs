// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use slint_interpreter::{
    ComponentHandle, LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Window,
};

use crate::errors::PyPlatformError;
use crate::geometry::{PyLogicalPosition, PyLogicalSize, PyPhysicalPosition, PyPhysicalSize};
use crate::image::PyImage;

/// A window that renders the scene of a component on the screen.
/// To get the window of a component, use `Component.window`.
///
/// **Note:** The window keeps its component alive only while it's shown.
/// Once the component is gone, all functions and properties of the window raise `RuntimeError`.
#[pyclass(unsendable, name = "Window")]
pub struct PyWindow {
    pub instance: slint_interpreter::Weak<slint_interpreter::ComponentInstance>,
}

impl PyWindow {
    fn instance(&self) -> PyResult<slint_interpreter::ComponentInstance> {
        self.instance.upgrade().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "The component of this window no longer exists",
            )
        })
    }

    fn with_window<R>(&self, f: impl FnOnce(&Window) -> R) -> PyResult<R> {
        Ok(f(self.instance()?.window()))
    }

    #[cfg(feature = "backend-testing")]
    pub fn dispatch_event(&self, event: i_slint_core::platform::WindowEvent) -> PyResult<()> {
        self.with_window(|window| window.dispatch_event(event))
    }
}

#[pymethods]
impl PyWindow {
    /// Shows the window on the screen.
    fn show(&self) -> PyResult<()> {
        Ok(self.instance()?.show().map_err(PyPlatformError)?)
    }

    /// Hides the window from the screen.
    fn hide(&self) -> PyResult<()> {
        Ok(self.instance()?.hide().map_err(PyPlatformError)?)
    }

    /// Takes a snapshot of the window contents and returns it as an image.
    /// To get a snapshot in the size of your component, show the window first.
    ///
    /// Raises `RuntimeError` if the backend or renderer can't take snapshots.
    ///
    /// **Note:** This function can be slow, because it re-renders the scene when needed.
    fn take_snapshot(&self) -> PyResult<PyImage> {
        let pixels = self.with_window(|window| window.take_snapshot())?.map_err(PyPlatformError)?;
        Ok(slint_interpreter::Image::from_rgba8(pixels).into())
    }

    /// Issues a request to the windowing system to re-render the contents of the window.
    fn request_redraw(&self) -> PyResult<()> {
        self.with_window(|window| window.request_redraw())
    }

    /// The factor to convert between logical and physical pixels.
    #[getter]
    fn scale_factor(&self) -> PyResult<f32> {
        self.with_window(|window| window.scale_factor())
    }

    /// The position of the window on the screen, in logical pixels and including the window frame.
    ///
    /// **Note:** Some windowing systems, such as Wayland, don't let you set the position;
    /// there, it always reads as `(0, 0)`.
    #[getter]
    fn logical_position(&self) -> PyResult<PyLogicalPosition> {
        self.with_window(|window| {
            let position = window.position().to_logical(window.scale_factor());
            PyLogicalPosition::new(position.x, position.y)
        })
    }

    #[setter]
    fn set_logical_position(&self, position: PyLogicalPosition) -> PyResult<()> {
        self.with_window(|window| window.set_position(LogicalPosition::new(position.x, position.y)))
    }

    /// The position of the window on the screen, in physical pixels and including the window frame.
    ///
    /// **Note:** Some windowing systems, such as Wayland, don't let you set the position;
    /// there, it always reads as `(0, 0)`.
    #[getter]
    fn physical_position(&self) -> PyResult<PyPhysicalPosition> {
        self.with_window(|window| {
            let position = window.position();
            PyPhysicalPosition::new(position.x, position.y)
        })
    }

    #[setter]
    fn set_physical_position(&self, position: PyPhysicalPosition) -> PyResult<()> {
        self.with_window(|window| {
            window.set_position(PhysicalPosition::new(position.x, position.y))
        })
    }

    /// The size of the window on the screen, in logical pixels and excluding the window frame.
    #[getter]
    fn logical_size(&self) -> PyResult<PyLogicalSize> {
        self.with_window(|window| {
            let size = window.size().to_logical(window.scale_factor());
            PyLogicalSize::new(size.width, size.height)
        })
    }

    #[setter]
    fn set_logical_size(&self, size: PyLogicalSize) -> PyResult<()> {
        self.with_window(|window| window.set_size(LogicalSize::new(size.width, size.height)))
    }

    /// The size of the window on the screen, in physical pixels and excluding the window frame.
    #[getter]
    fn physical_size(&self) -> PyResult<PyPhysicalSize> {
        self.with_window(|window| {
            let size = window.size();
            PyPhysicalSize::new(size.width, size.height)
        })
    }

    #[setter]
    fn set_physical_size(&self, size: PyPhysicalSize) -> PyResult<()> {
        self.with_window(|window| window.set_size(PhysicalSize::new(size.width, size.height)))
    }

    /// `True` if the window is visible on the screen; `False` otherwise.
    /// It can be `False` after `show()`, for example if the user minimized the window.
    #[getter]
    fn visible(&self) -> PyResult<bool> {
        self.with_window(|window| window.is_visible())
    }

    /// `True` if the window is in full screen mode; `False` otherwise.
    /// Set it to enter or leave full screen mode.
    #[getter]
    fn fullscreen(&self) -> PyResult<bool> {
        self.with_window(|window| window.is_fullscreen())
    }

    #[setter]
    fn set_fullscreen(&self, fullscreen: bool) -> PyResult<()> {
        self.with_window(|window| window.set_fullscreen(fullscreen))
    }

    /// `True` if the window is maximized; `False` otherwise.
    /// Set it to maximize the window or restore it.
    #[getter]
    fn maximized(&self) -> PyResult<bool> {
        self.with_window(|window| window.is_maximized())
    }

    #[setter]
    fn set_maximized(&self, maximized: bool) -> PyResult<()> {
        self.with_window(|window| window.set_maximized(maximized))
    }

    /// `True` if the window is minimized; `False` otherwise.
    /// Set it to minimize the window or restore it.
    #[getter]
    fn minimized(&self) -> PyResult<bool> {
        self.with_window(|window| window.is_minimized())
    }

    #[setter]
    fn set_minimized(&self, minimized: bool) -> PyResult<()> {
        self.with_window(|window| window.set_minimized(minimized))
    }
}
