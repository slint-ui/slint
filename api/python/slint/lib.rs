// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::{Cell, RefCell};

use pyo3_stub_gen::{define_stub_info_gatherer, derive::gen_stub_pyfunction};

mod image;
mod interpreter;
use interpreter::{
    CompilationResult, Compiler, ComponentDefinition, ComponentInstance, PyDiagnostic,
    PyDiagnosticLevel, PyValueType,
};
mod brush;
mod errors;
mod models;
mod timer;
mod value;

fn handle_unraisable(py: Python<'_>, context: String, err: PyErr) {
    let exception = err.value(py);
    let __notes__ = exception
        .getattr(pyo3::intern!(py, "__notes__"))
        .unwrap_or_else(|_| pyo3::types::PyList::empty(py).into_any());
    if let Ok(notes_list) = __notes__.downcast::<pyo3::types::PyList>() {
        let _ = notes_list.append(context);
        let _ = exception.setattr(pyo3::intern!(py, "__notes__"), __notes__);
    }

    if EVENT_LOOP_RUNNING.get() && err.is_instance_of::<pyo3::exceptions::PySystemExit>(py) {
        EVENT_LOOP_EXCEPTION.replace(Some(err));
        let _ = slint_interpreter::quit_event_loop();
    } else {
        err.write_unraisable(py, None);
    }
}

thread_local! {
    static EVENT_LOOP_RUNNING: Cell<bool> = Cell::new(false);
    static EVENT_LOOP_EXCEPTION: RefCell<Option<PyErr>> = RefCell::new(None)
}

#[gen_stub_pyfunction]
#[pyfunction]
fn run_event_loop() -> Result<(), PyErr> {
    EVENT_LOOP_EXCEPTION.replace(None);
    EVENT_LOOP_RUNNING.set(true);
    let result = slint_interpreter::run_event_loop();
    EVENT_LOOP_RUNNING.set(false);
    result.map_err(|e| errors::PyPlatformError::from(e))?;
    EVENT_LOOP_EXCEPTION.take().map_or(Ok(()), |err| Err(err))
}

#[gen_stub_pyfunction]
#[pyfunction]
fn quit_event_loop() -> Result<(), errors::PyEventLoopError> {
    slint_interpreter::quit_event_loop().map_err(|e| e.into())
}

#[gen_stub_pyfunction]
#[pyfunction]
fn set_xdg_app_id(app_id: String) -> Result<(), errors::PyPlatformError> {
    slint_interpreter::set_xdg_app_id(app_id).map_err(|e| e.into())
}

use pyo3::prelude::*;

#[pymodule]
fn slint(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    i_slint_backend_selector::with_platform(|_b| {
        // Nothing to do, just make sure a backend was created
        Ok(())
    })
    .map_err(|e| errors::PyPlatformError(e))?;

    m.add_class::<Compiler>()?;
    m.add_class::<CompilationResult>()?;
    m.add_class::<ComponentInstance>()?;
    m.add_class::<ComponentDefinition>()?;
    m.add_class::<image::PyImage>()?;
    m.add_class::<PyValueType>()?;
    m.add_class::<PyDiagnosticLevel>()?;
    m.add_class::<PyDiagnostic>()?;
    m.add_class::<timer::PyTimerMode>()?;
    m.add_class::<timer::PyTimer>()?;
    m.add_class::<brush::PyColor>()?;
    m.add_class::<brush::PyBrush>()?;
    m.add_class::<models::PyModelBase>()?;
    m.add_class::<value::PyStruct>()?;
    m.add_function(wrap_pyfunction!(run_event_loop, m)?)?;
    m.add_function(wrap_pyfunction!(quit_event_loop, m)?)?;
    m.add_function(wrap_pyfunction!(set_xdg_app_id, m)?)?;

    Ok(())
}

define_stub_info_gatherer!(stub_info);
