// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::sync::Arc;

use pyo3::prelude::*;

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
mod async_adapter;

use futures::future::FutureExt;

#[gen_stub_pyfunction]
#[pyfunction]
fn run_event_loop(/*slint_loop: pyo3::PyObject*/) -> Result<(), errors::PyPlatformError> {
    /*
        slint_interpreter::spawn_local(async move {
            loop {
                eprintln!("here");
                let mut readables = Vec::new();
                let mut writables = Vec::new();

                let timeout = Python::with_gil(|py| {
                    let bound_loop = slint_loop.bind(py);

                    py.import("asyncio.events").unwrap().getattr("_set_running_loop").unwrap().call1((bound_loop,)).unwrap();

                    bound_loop.call_method0("_run_once").unwrap();

                    py.import("asyncio.events").unwrap().getattr("_set_running_loop").unwrap().call1((py.None(),)).unwrap();

                    let selector = bound_loop.getattr("_selector").unwrap();

                    let timeout = selector
                        .getattr("next_timeout")
                        .and_then(|timeout_seconds_float| timeout_seconds_float.extract::<f64>())
                        .map(std::time::Duration::from_secs_f64)
                        .ok();

                    for fd_and_key_tuple in selector
                        .getattr("_fd_to_key")
                        .unwrap()
                        .downcast::<pyo3::types::PyMapping>()
                        .unwrap()
                        .items()
                        .unwrap()
                        .iter()
                    {
                        let tuple = fd_and_key_tuple.downcast::<pyo3::types::PyTuple>().unwrap();
                        let fd = tuple.get_item(0).unwrap();
                        let key = tuple.get_item(1).unwrap();

                        #[derive(Clone)]
                        struct FdWrapper(std::os::fd::RawFd);

                        impl std::os::fd::AsFd for FdWrapper {
                            fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
                                unsafe { std::os::fd::BorrowedFd::borrow_raw(self.0) }
                            }
                        }

                        let fd = FdWrapper(fd.extract::<std::os::fd::RawFd>().unwrap());
                        let events = key.getattr("events").unwrap().extract::<i32>().unwrap();

                        if events & 1 != 0 {
                            readables.push(
                                Arc::new(smol::Async::new(fd.clone()).unwrap())
                                    .readable_owned()
                                    .boxed_local(),
                            );
                        }
                        if events & 2 != 0 {
                            writables.push(
                                Arc::new(smol::Async::new(fd.clone()).unwrap())
                                    .writable_owned()
                                    .boxed_local(),
                            );
                        }
                    }

                    timeout
                });

                eprintln!(
                    "sleeping on {} readable and {} writable fds and timeout {:#?}",
                    readables.len(),
                    writables.len(),
                    timeout,
                );

                futures::select! {
                    woken_fd = futures::future::select_all(readables.into_iter().chain(writables)).fuse() => {
                        eprintln!("woken fd");
                    }
                    _ = timeout.map_or_else(|| smol::Timer::never(), |duration| smol::Timer::after(duration)).fuse() => {
                        eprintln!("timeout");
                    }
                }

                eprintln!("woke up from await")
            }
        })
        .unwrap();
    */

    slint_interpreter::run_event_loop().map_err(|e| e.into())
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

#[gen_stub_pyfunction]
#[pyfunction]
fn invoke_from_event_loop(callable: PyObject) -> Result<(), errors::PyEventLoopError> {
    slint_interpreter::invoke_from_event_loop(move || {
        Python::with_gil(|py| {
            if let Err(err) = callable.call0(py) {
                eprintln!("Error invoking python callable from closure invoked via slint::invoke_from_event_loop: {}", err)
            }
        })
    })
    .map_err(|e| e.into())
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
    m.add_class::<async_adapter::AsyncAdapter>()?;
    m.add_function(wrap_pyfunction!(run_event_loop, m)?)?;
    m.add_function(wrap_pyfunction!(quit_event_loop, m)?)?;
    m.add_function(wrap_pyfunction!(set_xdg_app_id, m)?)?;
    m.add_function(wrap_pyfunction!(invoke_from_event_loop, m)?)?;

    Ok(())
}

define_stub_info_gatherer!(stub_info);
