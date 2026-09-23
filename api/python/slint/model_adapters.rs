// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The model adapters of `i_slint_core::model` over Python models,
//! backing the adapter classes in `slint.models`.

use std::cell::{Cell, RefCell};
use std::pin::Pin;
use std::rc::Rc;

use i_slint_compiler::langtype::Type;
use i_slint_core::model::{
    MapModel, Model, ModelChangeListener, ModelChangeListenerBox, ModelRc, ModelTracker,
    ReverseModel,
};
use pyo3::PyTraverseError;
use pyo3::exceptions::{PyIndexError, PyTypeError};
use pyo3::gc::PyVisit;
use pyo3::prelude::*;

use crate::models::{ModelOwnership, PyModelBase, PyModelShared, ReadOnlyRustModel};
use crate::value::TypeCollection;

/// A Python object the adapter keeps alive, released by `__clear__`.
type PyObjectSlot = Rc<RefCell<Option<Py<PyAny>>>>;

/// Exceptions raised by the Python code an adapter calls.
#[derive(Default)]
struct Errors {
    active_calls: Cell<usize>,
    /// The first exception raised during a call, raised again to the Python caller of the adapter.
    pending: RefCell<Option<PyErr>>,
}

impl Errors {
    /// Records `err` for the active call, or reports it right away when it was
    /// raised outside a call, while the adapter handled a change notification.
    fn record(&self, py: Python<'_>, err: PyErr) {
        if self.active_calls.get() > 0 {
            self.pending.borrow_mut().get_or_insert(err);
        } else {
            crate::handle_unraisable(
                py,
                "Python: Model adapter caught an exception while handling a change notification"
                    .into(),
                err,
            );
        }
    }

    /// Runs `f` as a call of the adapter,
    /// returning the first exception the Python code raised during it.
    fn call<R>(&self, f: impl FnOnce() -> R) -> PyResult<R> {
        struct ActiveCall<'a>(&'a Cell<usize>);
        impl Drop for ActiveCall<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() - 1);
            }
        }

        self.active_calls.set(self.active_calls.get() + 1);
        let result = {
            let _active_call = ActiveCall(&self.active_calls);
            f()
        };
        match self.pending.borrow_mut().take() {
            Some(err) => Err(err),
            None => Ok(result),
        }
    }
}

type ErrorSlot = Rc<Errors>;

fn slot_object(py: Python<'_>, slot: &PyObjectSlot) -> Option<Py<PyAny>> {
    slot.borrow().as_ref().map(|obj| obj.clone_ref(py))
}

/// A Python `Model` as the source of an adapter.
struct PySourceModel {
    model: PyObjectSlot,
    shared: Rc<PyModelShared>,
    errors: ErrorSlot,
}

impl Model for PySourceModel {
    type Data = Py<PyAny>;

    fn row_count(&self) -> usize {
        Python::attach(|py| {
            let Some(model) = slot_object(py, &self.model) else { return 0 };
            match model.bind(py).call_method0("row_count").and_then(|count| count.extract()) {
                Ok(count) => count,
                Err(err) => {
                    self.errors.record(py, err);
                    0
                }
            }
        })
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        Python::attach(|py| {
            let model = slot_object(py, &self.model)?;
            match model.bind(py).call_method1("row_data", (row,)) {
                Ok(data) if data.is_none() => None,
                Ok(data) => Some(data.unbind()),
                Err(err) if err.is_instance_of::<PyIndexError>(py) => None,
                Err(err) => {
                    self.errors.record(py, err);
                    None
                }
            }
        })
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        Python::attach(|py| {
            let Some(model) = slot_object(py, &self.model) else { return };
            if let Err(err) = model.bind(py).call_method1("set_row_data", (row, data)) {
                self.errors.record(py, err);
            }
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.shared.notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// A model from Slint, such as the value of an array property, as the source of an adapter.
struct RustSourceModel {
    model: ModelRc<slint_interpreter::Value>,
    type_collection: TypeCollection,
    element_type: Option<Type>,
    errors: ErrorSlot,
}

impl Model for RustSourceModel {
    type Data = Py<PyAny>;

    fn row_count(&self) -> usize {
        self.model.row_count()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let value = self.model.row_data(row)?;
        Python::attach(|py| {
            match self
                .type_collection
                .to_py_value(value, self.element_type.clone())
                .into_pyobject(py)
            {
                Ok(data) => Some(data.unbind()),
                Err(err) => {
                    self.errors.record(py, err);
                    None
                }
            }
        })
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        let value = Python::attach(|py| {
            TypeCollection::slint_value_from_py_value(
                py,
                &data,
                Some(&self.type_collection),
                self.element_type.as_ref(),
            )
            .map_err(|err| self.errors.record(py, err))
        });
        if let Ok(value) = value {
            self.model.set_row_data(row, value);
        }
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.model.model_tracker()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// Wraps `source` for an adapter,
/// returning the Python objects the wrapper keeps alive.
fn source_model(
    source: &Bound<'_, PyAny>,
    errors: &ErrorSlot,
) -> PyResult<(ModelRc<Py<PyAny>>, Vec<PyObjectSlot>)> {
    if let Ok(base) = source.cast::<PyModelBase>() {
        let model: PyObjectSlot = Rc::new(RefCell::new(Some(source.clone().unbind())));
        let source = PySourceModel {
            model: model.clone(),
            shared: base.borrow().shared_model_for_adapter(),
            errors: errors.clone(),
        };
        return Ok((Rc::new(source).into(), vec![model]));
    }
    if let Ok(rust_model) = source.cast::<ReadOnlyRustModel>() {
        let rust_model = rust_model.borrow();
        let source = RustSourceModel {
            model: rust_model.model.clone(),
            type_collection: rust_model.type_collection.clone(),
            element_type: rust_model.element_type.clone(),
            errors: errors.clone(),
        };
        return Ok((Rc::new(source).into(), Vec::new()));
    }
    Err(PyTypeError::new_err(format!(
        "expected a slint.Model as source model, got {}",
        source.get_type().name()?
    )))
}

/// Forwards the adapter's change notifications to the views of the Python adapter class.
struct NotifyForwarder {
    target: Rc<RefCell<ModelOwnership>>,
}

impl NotifyForwarder {
    fn target(&self) -> Option<Rc<PyModelShared>> {
        self.target.borrow().shared_model()
    }
}

impl ModelChangeListener for NotifyForwarder {
    fn row_changed(self: Pin<&Self>, row: usize) {
        if let Some(target) = self.target() {
            target.notify.row_changed(row);
        }
    }

    fn row_added(self: Pin<&Self>, index: usize, count: usize) {
        if let Some(target) = self.target() {
            target.notify.row_added(index, count);
        }
    }

    fn row_removed(self: Pin<&Self>, index: usize, count: usize) {
        if let Some(target) = self.target() {
            target.notify.row_removed(index, count);
        }
    }

    fn reset(self: Pin<&Self>) {
        if let Some(target) = self.target() {
            target.notify.reset();
        }
    }
}

#[pyclass(unsendable, skip_from_py_object)]
pub struct PyModelAdapter {
    _forwarder: ModelChangeListenerBox<NotifyForwarder>,
    model: ModelRc<Py<PyAny>>,
    objects: Vec<PyObjectSlot>,
    errors: ErrorSlot,
}

impl PyModelAdapter {
    fn new(
        model: ModelRc<Py<PyAny>>,
        target: &PyModelBase,
        objects: Vec<PyObjectSlot>,
        errors: ErrorSlot,
    ) -> Self {
        let forwarder =
            ModelChangeListenerBox::new(NotifyForwarder { target: target.inner.clone() });
        model.model_tracker().attach_peer(forwarder.as_ref().model_peer());
        Self { _forwarder: forwarder, model, objects, errors }
    }
}

#[pymethods]
impl PyModelAdapter {
    /// A `MapModel` over `source`, notifying the views of `target`.
    #[staticmethod]
    fn map(
        source: &Bound<'_, PyAny>,
        map_function: Py<PyAny>,
        target: PyRef<'_, PyModelBase>,
    ) -> PyResult<Self> {
        let errors = ErrorSlot::default();
        let (source, mut objects) = source_model(source, &errors)?;
        let function: PyObjectSlot = Rc::new(RefCell::new(Some(map_function)));
        objects.push(function.clone());
        let function_errors = errors.clone();
        let model = MapModel::new(source, move |data: Py<PyAny>| {
            Python::attach(|py| {
                let Some(function) = slot_object(py, &function) else { return py.None() };
                match function.bind(py).call1((data,)) {
                    Ok(mapped) => mapped.unbind(),
                    Err(err) => {
                        function_errors.record(py, err);
                        py.None()
                    }
                }
            })
        });
        Ok(Self::new(Rc::new(model).into(), &target, objects, errors))
    }

    /// A `ReverseModel` over `source`, notifying the views of `target`.
    #[staticmethod]
    fn reverse(source: &Bound<'_, PyAny>, target: PyRef<'_, PyModelBase>) -> PyResult<Self> {
        let errors = ErrorSlot::default();
        let (source, objects) = source_model(source, &errors)?;
        let model = ReverseModel::new(source);
        Ok(Self::new(Rc::new(model).into(), &target, objects, errors))
    }

    fn row_count(&self) -> PyResult<usize> {
        self.errors.call(|| self.model.row_count())
    }

    fn row_data(&self, row: usize) -> PyResult<Option<Py<PyAny>>> {
        self.errors.call(|| self.model.row_data(row))
    }

    fn set_row_data(&self, row: usize, data: Py<PyAny>) -> PyResult<()> {
        if row >= self.row_count()? {
            return Err(PyIndexError::new_err("row index out of range"));
        }
        self.errors.call(|| self.model.set_row_data(row, data))
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for slot in &self.objects {
            if let Some(obj) = slot.try_borrow().ok().as_deref().and_then(Option::as_ref) {
                visit.call(obj)?;
            }
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        for slot in &self.objects {
            slot.borrow_mut().take();
        }
    }
}
