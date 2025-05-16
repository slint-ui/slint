// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::rc::Rc;

use i_slint_core::model::{Model, ModelNotify, ModelRc};

use pyo3::exceptions::PyIndexError;
use pyo3::gc::PyVisit;
use pyo3::prelude::*;
use pyo3::PyTraverseError;

use crate::value::PyValue;

pub struct PyModelShared {
    notify: ModelNotify,
    self_ref: RefCell<Option<PyObject>>,
}

#[derive(Clone)]
#[pyclass(unsendable, weakref, subclass)]
pub struct PyModelBase {
    inner: Rc<PyModelShared>,
}

impl PyModelBase {
    pub fn as_model(&self) -> ModelRc<slint_interpreter::Value> {
        self.inner.clone().into()
    }
}

#[pymethods]
impl PyModelBase {
    #[new]
    fn new() -> Self {
        Self {
            inner: Rc::new(PyModelShared {
                notify: Default::default(),
                self_ref: RefCell::new(None),
            }),
        }
    }

    fn init_self(&self, self_ref: PyObject) {
        *self.inner.self_ref.borrow_mut() = Some(self_ref);
    }

    fn notify_row_added(&self, index: usize, count: usize) {
        self.inner.notify.row_added(index, count)
    }

    fn notify_row_changed(&self, index: usize) {
        self.inner.notify.row_changed(index)
    }

    fn notify_row_removed(&self, index: usize, count: usize) {
        self.inner.notify.row_removed(index, count)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Some(this) = self.inner.self_ref.borrow().as_ref() {
            visit.call(this)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        *self.inner.self_ref.borrow_mut() = None;
    }
}

impl i_slint_core::model::Model for PyModelShared {
    type Data = slint_interpreter::Value;

    fn row_count(&self) -> usize {
        Python::with_gil(|py| {
            let obj = self.self_ref.borrow();
            let Some(obj) = obj.as_ref() else {
                eprintln!("Python: Model implementation is lacking self object (in row_count)");
                return 0;
            };
            let result = match obj.call_method0(py, "row_count") {
                Ok(result) => result,
                Err(err) => {
                    eprintln!(
                        "Python: Model implementation of row_count() threw an exception: {err}"
                    );
                    return 0;
                }
            };

            match result.extract::<usize>(py) {
                Ok(count) => count,
                Err(err) => {
                    eprintln!("Python: Model implementation of row_count() returned value that cannot be cast to usize: {err}");
                    0
                }
            }
        })
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        Python::with_gil(|py| {
            let obj = self.self_ref.borrow();
            let Some(obj) = obj.as_ref() else {
                eprintln!("Python: Model implementation is lacking self object (in row_data)");
                return None;
            };

            let result = match obj.call_method1(py, "row_data", (row,)) {
                Ok(result) => result,
                Err(err) if err.is_instance_of::<PyIndexError>(py) => return None,
                Err(err) => {
                    eprintln!(
                        "Python: Model implementation of row_data() threw an exception: {err}"
                    );
                    return None;
                }
            };

            match result.extract::<PyValue>(py) {
                Ok(pv) => Some(pv.0),
                Err(err) => {
                    eprintln!("Python: Model implementation of row_data() returned value that cannot be converted to Rust: {err}");
                    None
                }
            }
        })
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        Python::with_gil(|py| {
            let obj = self.self_ref.borrow();
            let Some(obj) = obj.as_ref() else {
                eprintln!("Python: Model implementation is lacking self object (in set_row_data)");
                return;
            };

            if let Err(err) = obj.call_method1(py, "set_row_data", (row, PyValue::from(data))) {
                eprintln!(
                    "Python: Model implementation of set_row_data() threw an exception: {err}"
                );
            };
        });
    }

    fn model_tracker(&self) -> &dyn i_slint_core::model::ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl PyModelShared {
    pub fn rust_into_py_model<'py>(
        model: &ModelRc<slint_interpreter::Value>,
        py: Python<'py>,
    ) -> Option<Bound<'py, PyAny>> {
        model.as_any().downcast_ref::<PyModelShared>().and_then(|rust_model| {
            rust_model.self_ref.borrow().as_ref().map(|obj| obj.clone_ref(py).into_bound(py))
        })
    }
}

#[pyclass(unsendable)]
pub struct ReadOnlyRustModel(pub ModelRc<slint_interpreter::Value>);

#[pymethods]
impl ReadOnlyRustModel {
    fn row_count(&self) -> usize {
        self.0.row_count()
    }

    fn row_data(&self, row: usize) -> Option<PyValue> {
        self.0.row_data(row).map(|value| value.into())
    }

    fn __len__(&self) -> usize {
        self.row_count()
    }

    fn __iter__(slf: PyRef<'_, Self>) -> ReadOnlyRustModelIterator {
        ReadOnlyRustModelIterator { model: slf.0.clone(), row: 0 }
    }

    fn __getitem__(&self, index: usize) -> Option<PyValue> {
        self.row_data(index)
    }
}

impl From<&ModelRc<slint_interpreter::Value>> for ReadOnlyRustModel {
    fn from(model: &ModelRc<slint_interpreter::Value>) -> Self {
        Self(model.clone())
    }
}

#[pyclass(unsendable)]
struct ReadOnlyRustModelIterator {
    model: ModelRc<slint_interpreter::Value>,
    row: usize,
}

#[pymethods]
impl ReadOnlyRustModelIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<PyValue> {
        if self.row >= self.model.row_count() {
            return None;
        }
        let row = self.row;
        self.row += 1;
        self.model.row_data(row).map(|value| value.into())
    }
}
