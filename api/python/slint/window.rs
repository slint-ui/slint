// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::window::WindowAdapterRc;
use pyo3::prelude::*;

use crate::errors::PyPlatformError;
use crate::image::PyImage;

/// A window that renders the scene of a component on the screen.
/// Use `Component.window` to get the window of a component.
#[pyclass(unsendable, name = "Window")]
pub struct PyWindow {
    pub adapter: WindowAdapterRc,
}

#[pymethods]
impl PyWindow {
    /// Takes a snapshot of the window contents and returns it as an image.
    /// Raises `RuntimeError` if the backend or renderer can't take snapshots.
    ///
    /// This function may be slow, as it may need to re-render the scene.
    fn take_snapshot(&self) -> Result<PyImage, PyPlatformError> {
        let pixels = self.adapter.window().take_snapshot()?;
        Ok(slint_interpreter::Image::from_rgba8(pixels).into())
    }
}
