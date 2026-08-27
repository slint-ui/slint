// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::PyTraverseError;
use pyo3::gc::PyVisit;
use pyo3::prelude::*;

use std::any::Any;
use std::rc::Rc;

/// Python representation of some form of type-indexed possibly-lazy data transfer.
/// Used for accessing the platform clipboard and drag-and-drop APIs.
#[pyclass(unsendable, name = "DataTransfer", skip_from_py_object)]
#[derive(Clone)]
pub struct PyDataTransfer {
    pub data_transfer: i_slint_core::data_transfer::DataTransfer,
}

#[pymethods]
impl PyDataTransfer {
    /// Constructs an empty `DataTransfer`.
    #[new]
    fn new() -> Self {
        Self { data_transfer: Default::default() }
    }

    /// The plain text representation of this `DataTransfer`, or `None` if no plain text
    /// is available. Assigning `None` or the empty string clears any previously-set
    /// plain text; assigning any other string overwrites it.
    #[getter]
    fn plain_text(&self) -> Option<String> {
        self.data_transfer.plain_text().ok().map(|s| s.to_string())
    }

    /// Sets the plain text representation of this `DataTransfer`.
    ///
    /// Assigning `None` or the empty string clears any previously-set plain text;
    /// assigning any other string overwrites it.
    #[setter]
    fn set_plain_text(&mut self, text: Option<&str>) {
        self.data_transfer.set_plain_text(text.unwrap_or_default().into());
    }

    /// `True` if this `DataTransfer` advertises a plain text representation.
    #[getter]
    fn has_plain_text(&self) -> bool {
        self.data_transfer.has_plain_text()
    }

    /// The image representation of this `DataTransfer`, or `None` if no image is
    /// available. Assigning `None` clears any previously-set image; assigning any
    /// other image overwrites it.
    #[getter]
    fn image(&self) -> Option<crate::image::PyImage> {
        self.data_transfer.image().ok().map(crate::image::PyImage::from)
    }

    /// Sets the image representation of this `DataTransfer`.
    ///
    /// Assigning `None` clears any previously-set image; assigning any other image
    /// overwrites it.
    #[setter]
    fn set_image(&mut self, image: Option<&crate::image::PyImage>) {
        self.data_transfer.set_image(image.map(|i| i.image.clone()).unwrap_or_default());
    }

    /// `True` if this `DataTransfer` advertises an image representation.
    #[getter]
    fn has_image(&self) -> bool {
        self.data_transfer.has_image()
    }

    /// `True` if this `DataTransfer` carries no data: no plain text, no image, and no
    /// user data.
    #[getter]
    fn is_empty(&self) -> bool {
        self.data_transfer.is_empty()
    }

    /// Application-internal user data attached to this `DataTransfer`. Use this when the
    /// drag-and-drop or clipboard operation stays inside the current Python application and you
    /// want to avoid serializing to plain text or an image.
    ///
    /// Reading returns the Python object previously assigned, or `None` if none was set (or the
    /// user data was set by a non-Python binding). Assigning `None` clears any previously
    /// attached Python user data.
    #[getter]
    fn user_data(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        let any = self.data_transfer.user_data()?;
        let py_any = any.downcast::<Py<PyAny>>().ok()?;
        Some((*py_any).clone_ref(py))
    }

    #[setter]
    fn set_user_data(&mut self, value: Bound<'_, PyAny>) {
        if value.is_none() {
            // The underlying field is private; install a sentinel that fails the
            // `Py<PyAny>` downcast so the property reads back as `None`.
            self.data_transfer.set_user_data(Rc::new(()) as Rc<dyn Any>);
        } else {
            self.data_transfer.set_user_data(Rc::new(value.unbind()) as Rc<dyn Any>);
        }
    }

    fn __repr__(&self) -> String {
        format!("DataTransfer({:?})", self.data_transfer)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.data_transfer == other.data_transfer
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Some(any) = self.data_transfer.user_data() {
            if let Some(py_any) = (*any).downcast_ref::<Py<PyAny>>() {
                visit.call(py_any)?;
            }
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        // Drop our reference to the Python user-data by installing the same
        // sentinel the setter uses for `None`. If no other Rust clone shares
        // this `Rc<dyn Any>`, the inner `Py<PyAny>` is released here.
        self.data_transfer.set_user_data(Rc::new(()) as Rc<dyn Any>);
    }
}

impl From<i_slint_core::data_transfer::DataTransfer> for PyDataTransfer {
    fn from(data_transfer: i_slint_core::data_transfer::DataTransfer) -> Self {
        Self { data_transfer }
    }
}
