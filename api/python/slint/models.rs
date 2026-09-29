// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore getitem unraisable
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use i_slint_compiler::langtype::Type;
use i_slint_core::model::{Model, ModelError, ModelNotify, ModelRc};

use pyo3::PyTraverseError;
use pyo3::exceptions::{PyIndexError, PyNotImplementedError};
use pyo3::gc::PyVisit;
use pyo3::prelude::*;

use crate::value::{SlintToPyValue, TypeCollection};

#[derive(Default)]
pub struct PyModelShared {
    notify: ModelNotify,
    self_ref: RefCell<Option<Py<PyAny>>>,
    /// The type collection is needed when calling a Python implementation of set_row_data and
    /// the model data provided (for example from within a .slint file) contains an enum. Then
    /// we need to know how to map it to the correct Python enum. This field is lazily set, whenever
    /// time the Python model is exposed to Slint.
    type_collection: RefCell<Option<TypeCollection>>,
    /// Element type of the model, used in `set_row_data` to preserve `int` vs `float`
    /// when slint code writes a row back into the Python model.
    element_type: RefCell<Option<Type>>,
}

impl PyModelShared {
    /// Let the cyclic GC see the wrapper this shared model keeps alive.
    pub fn visit_wrapper(&self, visit: &PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Some(wrapper) = self.self_ref.borrow().as_ref() {
            visit.call(wrapper)?;
        }
        Ok(())
    }

    /// Drop the strong reference to the wrapper when its holder is cleared.
    /// If two wrappers share one model, this also breaks the survivor.
    pub fn clear_self_ref(&self) {
        *self.self_ref.borrow_mut() = None;
    }

    /// The wrapper object to call back into, or `None` (after logging) if unset.
    fn wrapper_obj<'py>(&self, py: Python<'py>, caller: &str) -> Option<Bound<'py, PyAny>> {
        let obj = self.self_ref.borrow().as_ref().map(|obj| obj.clone_ref(py).into_bound(py));
        if obj.is_none() {
            eprintln!("Python: Model implementation is lacking self object (in {caller})");
        }
        obj
    }

    pub fn apply_type_collection(
        &self,
        type_collection: &TypeCollection,
        element_type: Option<Type>,
    ) {
        if let Ok(mut type_collection_ref) = self.type_collection.try_borrow_mut() {
            *type_collection_ref = Some(type_collection.clone());
        }
        if let Ok(mut element_type_ref) = self.element_type.try_borrow_mut() {
            *element_type_ref = element_type;
        }
    }
}

/// Ownership of the shared model, from the Python wrapper's point of view.
enum ModelOwnership {
    OwnedByWrapper(Rc<PyModelShared>),
    OwnedBySlint(Weak<PyModelShared>),
}

#[pyclass(unsendable, weakref, subclass, skip_from_py_object)]
pub struct PyModelBase {
    inner: RefCell<ModelOwnership>,
}

impl PyModelBase {
    fn shared_model(&self) -> Option<Rc<PyModelShared>> {
        match &*self.inner.borrow() {
            ModelOwnership::OwnedByWrapper(shared) => Some(shared.clone()),
            ModelOwnership::OwnedBySlint(weak) => weak.upgrade(),
        }
    }

    /// Move ownership of the shared model to Slint; the wrapper keeps only a
    /// weak reference. Re-hand-off after Slint dropped the `ModelRc` attaches
    /// a fresh shared model; `self_ref` is only set when still empty.
    pub fn hand_to_slint(&self, wrapper: &Bound<'_, PyAny>) -> ModelRc<slint_interpreter::Value> {
        let shared = self.shared_model().unwrap_or_else(|| Rc::new(PyModelShared::default()));
        *self.inner.borrow_mut() = ModelOwnership::OwnedBySlint(Rc::downgrade(&shared));
        {
            let mut self_ref = shared.self_ref.borrow_mut();
            if self_ref.is_none() {
                *self_ref = Some(wrapper.clone().unbind());
            }
        }
        shared.into()
    }
}

#[pymethods]
impl PyModelBase {
    #[new]
    fn new() -> Self {
        Self {
            inner: RefCell::new(ModelOwnership::OwnedByWrapper(Rc::new(PyModelShared::default()))),
        }
    }

    // The notifications are no-ops once Slint dropped the last ModelRc of this model (the
    // weak reference is dead): there are no views attached anymore to notify. The wrapper
    // stays usable, and handing the model to Slint again re-attaches a fresh shared model.
    fn notify_row_added(&self, index: usize, count: usize) {
        if let Some(shared) = self.shared_model() {
            shared.notify.row_added(index, count)
        }
    }

    fn notify_row_changed(&self, index: usize) {
        if let Some(shared) = self.shared_model() {
            shared.notify.row_changed(index)
        }
    }

    fn notify_row_removed(&self, index: usize, count: usize) {
        if let Some(shared) = self.shared_model() {
            shared.notify.row_removed(index, count)
        }
    }
}

impl i_slint_core::model::Model for PyModelShared {
    type Data = slint_interpreter::Value;

    fn row_count(&self) -> usize {
        Python::try_attach(|py| {
            let Some(obj) = self.wrapper_obj(py, "row_count") else { return 0; };
            let result = match obj.call_method0("row_count") {
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

            match result.extract::<usize>() {
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
        }).unwrap_or_default()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        Python::try_attach(|py| {
            let Some(obj) = self.wrapper_obj(py, "row_data") else { return None; };

            let result = match obj.call_method1("row_data", (row,)) {
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
                &result.clone().unbind(),
                self.type_collection.borrow().as_ref(),
                None,
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
        }).flatten()
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        Python::try_attach(|py| {
            let Some(obj) = self.wrapper_obj(py, "set_row_data") else {
                return;
            };

            let Some(type_collection) = self.type_collection.borrow().as_ref().cloned() else {
                eprintln!(
                    "Python: Model implementation is lacking type collection (in set_row_data)"
                );
                return;
            };

            let element_type = self.element_type.borrow().clone();
            if let Err(err) = obj.call_method1(
                "set_row_data",
                (row, type_collection.to_py_value(data, element_type)),
            ) {
                crate::handle_unraisable(
                    py,
                    "Python: Model implementation of set_row_data() threw an exception".into(),
                    err,
                );
            };
        });
    }

    fn push_row(&self, data: Self::Data) -> Result<(), ModelError> {
        Python::try_attach(|py| {
            let Some(obj) = self.wrapper_obj(py, "push_row") else {
                return Err(ModelError::unsupported(self));
            };

            let Some(type_collection) = self.type_collection.borrow().as_ref().cloned() else {
                eprintln!("Python: Model implementation is lacking type collection (in push_row)");
                return Err(ModelError::unsupported(self));
            };

            let element_type = self.element_type.borrow().clone();
            let result =
                obj.call_method1("push_row", (type_collection.to_py_value(data, element_type),));
            self.map_result(py, result, "push_row()")
        })
        .unwrap_or(Err(ModelError::unsupported(self)))
    }

    fn remove_row(&self, row: usize) -> Result<(), ModelError> {
        let row_count = self.row_count();
        if row >= row_count {
            return Err(ModelError::out_of_bounds(row_count));
        }

        Python::try_attach(|py| {
            let Some(obj) = self.wrapper_obj(py, "remove_row") else {
                return Err(ModelError::unsupported(self));
            };

            let result = obj.call_method1("remove_row", (row,));
            self.map_result(py, result, "remove_row()")
        })
        .unwrap_or(Err(ModelError::unsupported(self)))
    }

    fn insert_row(&self, row: usize, data: Self::Data) -> Result<(), ModelError> {
        let row_count = self.row_count();
        if row > row_count {
            return Err(ModelError::out_of_bounds(row_count));
        }

        Python::try_attach(|py| {
            let Some(obj) = self.wrapper_obj(py, "insert_row") else {
                return Err(ModelError::unsupported(self));
            };

            let Some(type_collection) = self.type_collection.borrow().as_ref().cloned() else {
                eprintln!(
                    "Python: Model implementation is lacking type collection (in insert_row)"
                );
                return Err(ModelError::unsupported(self));
            };

            let element_type = self.element_type.borrow().clone();
            let result = obj
                .call_method1("insert_row", (row, type_collection.to_py_value(data, element_type)));
            self.map_result(py, result, "insert_row()")
        })
        .unwrap_or(Err(ModelError::unsupported(self)))
    }

    fn model_tracker(&self) -> &dyn i_slint_core::model::ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl PyModelShared {
    /// Maps the result of calling a Python row modification method to a ModelError.
    ///
    /// A rejected modification is reported by raising: IndexError for a row that
    /// is out of bounds and NotImplementedError for an unsupported modification.
    /// Unexpected exceptions are also reported through the unraisable hook.
    fn map_result(
        &self,
        py: Python<'_>,
        result: PyResult<Bound<'_, PyAny>>,
        function: &str,
    ) -> Result<(), ModelError> {
        match result {
            Ok(_) => Ok(()),
            Err(err) if err.is_instance_of::<PyIndexError>(py) => {
                Err(ModelError::out_of_bounds(self.row_count()))
            }
            Err(err) if err.is_instance_of::<PyNotImplementedError>(py) => {
                Err(self.unsupported_error(py))
            }
            Err(err) => {
                crate::handle_unraisable(
                    py,
                    format!("Python: Model implementation of {function} threw an exception"),
                    err,
                );
                Err(self.unsupported_error(py))
            }
        }
    }

    /// An unsupported ModelError naming the Python type of the model, falling
    /// back to the name of this wrapper.
    fn unsupported_error(&self, py: Python<'_>) -> ModelError {
        let python_type_name = || -> Option<String> {
            let obj = self.self_ref.borrow();
            Some(obj.as_ref()?.bind(py).get_type().name().ok()?.to_string())
        };
        match python_type_name() {
            Some(name) => ModelError::unsupported_by_name(name, i_slint_core::InternalToken),
            None => ModelError::unsupported(self),
        }
    }

    pub fn rust_into_py_model<'py>(
        model: &ModelRc<slint_interpreter::Value>,
        py: Python<'py>,
    ) -> Option<Bound<'py, PyAny>> {
        model
            .as_any()
            .downcast_ref::<PyModelShared>()
            .and_then(|rust_model| rust_model.wrapper_obj(py, "rust_into_py_model"))
    }
}

#[pyclass(unsendable)]
pub struct ReadOnlyRustModel {
    pub model: ModelRc<slint_interpreter::Value>,
    pub type_collection: TypeCollection,
    /// The declared element type (e.g. the `T` of `[T]`), when known. Used so
    /// row access maps each value to the correct Python type.
    pub element_type: Option<Type>,
}

#[pymethods]
impl ReadOnlyRustModel {
    fn row_count(&self) -> usize {
        self.model.row_count()
    }

    fn row_data(&self, row: usize) -> Option<SlintToPyValue> {
        self.model
            .row_data(row)
            .map(|value| self.type_collection.to_py_value(value, self.element_type.clone()))
    }

    fn __len__(&self) -> usize {
        self.row_count()
    }

    fn __iter__(slf: PyRef<'_, Self>) -> ReadOnlyRustModelIterator {
        ReadOnlyRustModelIterator {
            model: slf.model.clone(),
            row: 0,
            type_collection: slf.type_collection.clone(),
            element_type: slf.element_type.clone(),
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
    element_type: Option<Type>,
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
        self.model
            .row_data(row)
            .map(|value| self.type_collection.to_py_value(value, self.element_type.clone()))
    }
}
