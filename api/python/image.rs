// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;

#[pyclass(unsendable)]
pub struct PyImage {
    pub image: slint_interpreter::Image,
}

#[pymethods]
impl PyImage {
    #[new]
    fn py_new() -> PyResult<Self> {
        Ok(Self { image: Default::default() })
    }

    #[getter]
    fn size(&self) -> PyResult<(u32, u32)> {
        Ok(self.image.size().into())
    }

    #[getter]
    fn width(&self) -> PyResult<u32> {
        Ok(self.image.size().width)
    }

    #[getter]
    fn height(&self) -> PyResult<u32> {
        Ok(self.image.size().height)
    }

    #[getter]
    fn path(&self) -> PyResult<Option<&std::path::Path>> {
        Ok(self.image.path())
    }

    #[staticmethod]
    fn load_from_path(path: std::path::PathBuf) -> Result<Self, crate::errors::PyLoadImageError> {
        let image = slint_interpreter::Image::load_from_path(&path)?;
        Ok(Self { image })
    }

    #[staticmethod]
    fn load_from_svg_data(data: &[u8]) -> Result<Self, crate::errors::PyLoadImageError> {
        let image = slint_interpreter::Image::load_from_svg_data(data)?;
        Ok(Self { image })
    }
}

impl From<&slint_interpreter::Image> for PyImage {
    fn from(image: &slint_interpreter::Image) -> Self {
        Self { image: image.clone() }
    }
}
