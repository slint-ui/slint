// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::rc::Rc;

use i_slint_core::model::{Model, ModelNotify, ModelRc};

use pyo3::exceptions::PyIndexError;
use pyo3::gc::PyVisit;
use pyo3::prelude::*;
use pyo3::PyTraverseError;

use crate::value::{SlintToPyValue, TypeCollection};

pub struct PyModelShared {
    notify: ModelNotify,
    self_ref: RefCell<Option<Py<PyAny>>>,
    /// The type collection is needed when calling a Python implementation of set_row_data and
    /// the model data provided (for example from within a .slint file) contains an enum. Then
    /// we need to know how to map it to the correct Python enum. This field is lazily set, whenever
    /// time the Python model is exposed to Slint.
    type_collection: RefCell<Option<TypeCollection>>,
}

impl PyModelShared {
    pub fn apply_type_collection(&self, type_collection: &TypeCollection) {
        if let Ok(mut type_collection_ref) = self.type_collection.try_borrow_mut() {
            *type_collection_ref = Some(type_collection.clone());
        }
    }
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
                type_collection: RefCell::new(None),
            }),
        }
    }

    fn init_self(&self, self_ref: Py<PyAny>) {
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
        Python::attach(|py| {
            let obj = self.self_ref.borrow();
            let Some(obj) = obj.as_ref() else {
                eprintln!("Python: Model implementation is lacking self object (in row_count)");
                return 0;
            };
            let result = match obj.call_method0(py, "row_count") {
                Ok(result) => result,
                Err(err) => {
                    crate::handle_unraisable(
                        py,
                        "Python: Model implementation of row_count() threw an exception".into(),
                        err,
                    );
                    return 0;
                }
            };

            match result.extract::<usize>(py) {
                Ok(count) => count,
                Err(err) => {
                    crate::handle_unraisable(
                        py,
                        "Python: Model implementation of row_count() returned value that cannot be cast to usize".into(),
                        err,
                    );
                    0
                }
            }
        })
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        Python::attach(|py| {
            let obj = self.self_ref.borrow();
            let Some(obj) = obj.as_ref() else {
                eprintln!("Python: Model implementation is lacking self object (in row_data)");
                return None;
            };

            let result = match obj.call_method1(py, "row_data", (row,)) {
                Ok(result) => result,
                Err(err) if err.is_instance_of::<PyIndexError>(py) => return None,
                Err(err) => {
                    crate::handle_unraisable(
                        py,
                        "Python: Model implementation of row_data() threw an exception".into(),
                        err,
                    );
                    return None;
                }
            };

            match TypeCollection::slint_value_from_py_value(
                py,
                &result,
                self.type_collection.borrow().as_ref(),
            ) {
                Ok(pv) => Some(pv),
                Err(err) => {
                    crate::handle_unraisable(
                        py,
                        "Python: Model implementation of row_data() returned value that cannot be cast to usize".into(),
                        err,
                    );
                    None
                }
            }
        })
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        Python::attach(|py| {
            let obj = self.self_ref.borrow();
            let Some(obj) = obj.as_ref() else {
                eprintln!("Python: Model implementation is lacking self object (in set_row_data)");
                return;
            };

            let Some(type_collection) = self.type_collection.borrow().as_ref().cloned() else {
                eprintln!(
                    "Python: Model implementation is lacking type collection (in set_row_data)"
                );
                return;
            };

            if let Err(err) =
                obj.call_method1(py, "set_row_data", (row, type_collection.to_py_value(data)))
            {
                crate::handle_unraisable(
                    py,
                    "Python: Model implementation of set_row_data() threw an exception".into(),
                    err,
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
pub struct ReadOnlyRustModel {
    pub model: ModelRc<slint_interpreter::Value>,
    pub type_collection: TypeCollection,
}

#[pymethods]
impl ReadOnlyRustModel {
    fn row_count(&self) -> usize {
        self.model.row_count()
    }

    fn row_data(&self, row: usize) -> Option<SlintToPyValue> {
        self.model.row_data(row).map(|value| self.type_collection.to_py_value(value))
    }

    fn __len__(&self) -> usize {
        self.row_count()
    }

    fn __iter__(slf: PyRef<'_, Self>) -> ReadOnlyRustModelIterator {
        ReadOnlyRustModelIterator {
            model: slf.model.clone(),
            row: 0,
            type_collection: slf.type_collection.clone(),
        }
    }

    fn __getitem__(&self, index: usize) -> Option<SlintToPyValue> {
        self.row_data(index)
    }
}

#[pyclass(unsendable)]
struct ReadOnlyRustModelIterator {
    model: ModelRc<slint_interpreter::Value>,
    row: usize,
    type_collection: TypeCollection,
}

#[pymethods]
impl ReadOnlyRustModelIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<SlintToPyValue> {
        if self.row >= self.model.row_count() {
            return None;
        }
        let row = self.row;
        self.row += 1;
        self.model.row_data(row).map(|value| self.type_collection.to_py_value(value))
    }
}
