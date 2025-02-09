// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use pyo3_stub_gen::{derive::gen_stub_pyclass, derive::gen_stub_pymethods};

/// Image objects can be set on Slint Image elements for display. Construct Image objects from a path to an
/// image file on disk, using `Image.load_from_path`.
#[gen_stub_pyclass]
#[pyclass(unsendable, name = "Image")]
pub struct PyImage {
    pub image: slint_interpreter::Image,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyImage {
    #[new]
    fn py_new() -> PyResult<Self> {
        Ok(Self { image: Default::default() })
    }

    /// The size of the image as tuple of `width` and `height`.
    #[getter]
    fn size(&self) -> PyResult<(u32, u32)> {
        Ok(self.image.size().into())
    }

    /// The width of the image in pixels.
    #[getter]
    fn width(&self) -> PyResult<u32> {
        Ok(self.image.size().width)
    }

    /// The height of the image in pixels.
    #[getter]
    fn height(&self) -> PyResult<u32> {
        Ok(self.image.size().height)
    }

    /// The path of the image if it was loaded from disk, or None.
    #[getter]
    fn path(&self) -> PyResult<Option<std::path::PathBuf>> {
        Ok(self.image.path().map(|p| p.to_path_buf()))
    }

    /// Loads the image from the specified path. Returns None if the image can't be loaded.
    #[staticmethod]
    fn load_from_path(path: std::path::PathBuf) -> Result<Self, crate::errors::PyLoadImageError> {
        let image = slint_interpreter::Image::load_from_path(&path)?;
        Ok(Self { image })
    }

    /// Creates a new image from a string that describes the image in SVG format.
    #[staticmethod]
    fn load_from_svg_data(data: Vec<u8>) -> Result<Self, crate::errors::PyLoadImageError> {
        let image = slint_interpreter::Image::load_from_svg_data(&data)?;
        Ok(Self { image })
    }
}

impl From<&slint_interpreter::Image> for PyImage {
    fn from(image: &slint_interpreter::Image) -> Self {
        Self { image: image.clone() }
    }
}
