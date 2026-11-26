// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use slint_interpreter::ComponentHandle;

use crate::errors::PyPlatformError;
use crate::image::PyImage;

/// A window that renders the scene of a component on the screen.
/// To get the window of a component, use `Component.window`.
#[pyclass(unsendable, name = "Window")]
pub struct PyWindow {
    pub instance: slint_interpreter::Weak<slint_interpreter::ComponentInstance>,
}

#[pymethods]
impl PyWindow {
    /// Takes a snapshot of the window contents and returns it as an image.
    /// To get a snapshot in the size of your component, show the window first.
    ///
    /// Raises `RuntimeError` if the backend or renderer can't take snapshots,
    /// or if the component of the window no longer exists.
    ///
    /// **Note:** This function can be slow, because it re-renders the scene when needed.
    fn take_snapshot(&self) -> PyResult<PyImage> {
        let instance = self.instance.upgrade().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "The component of this window no longer exists",
            )
        })?;
        let pixels = instance.window().take_snapshot().map_err(PyPlatformError)?;
        Ok(slint_interpreter::Image::from_rgba8_straight(pixels.assume_straight()).into())
    }
}
