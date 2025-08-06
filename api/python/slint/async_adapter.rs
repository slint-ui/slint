// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3_stub_gen::{
    derive::gen_stub_pyclass, derive::gen_stub_pyclass_enum, derive::gen_stub_pymethods,
};

struct PyFdWrapper(std::os::fd::RawFd);

impl std::os::fd::AsFd for PyFdWrapper {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        unsafe { std::os::fd::BorrowedFd::borrow_raw(self.0) }
    }
}

#[gen_stub_pyclass]
#[pyclass(unsendable)]
pub struct AsyncAdapter {
    adapter: Arc<smol::Async<PyFdWrapper>>,
    read_handle: Option<slint_interpreter::JoinHandle<()>>,
    write_handle: Option<slint_interpreter::JoinHandle<()>>,
}

#[gen_stub_pymethods]
#[pymethods]
impl AsyncAdapter {
    #[new]
    fn py_new(fd: i32) -> Self {
        AsyncAdapter {
            adapter: Arc::new(smol::Async::new(PyFdWrapper(fd)).unwrap()),
            read_handle: None,
            write_handle: None,
        }
    }

    fn wait_for_readable(&mut self, callback: PyObject) {
        let adapter = self.adapter.clone().readable_owned();
        let fd = self.adapter.get_ref().0;
        self.read_handle = Some(
            slint_interpreter::spawn_local(async move {
                adapter.await;
                Python::with_gil(|py| {
                    callback
                        .call1(py, (fd,))
                        .expect("unexpected failure running python async adapter callback");
                });
            })
            .unwrap(),
        );
    }

    fn wait_for_writable(&mut self, callback: PyObject) {
        let adapter = self.adapter.clone().writable_owned();
        let fd = self.adapter.get_ref().0;
        self.write_handle = Some(
            slint_interpreter::spawn_local(async move {
                adapter.await;
                Python::with_gil(|py| {
                    callback
                        .call1(py, (fd,))
                        .expect("unexpected failure running python async adapter callback");
                });
            })
            .unwrap(),
        );
    }
}

impl Drop for AsyncAdapter {
    fn drop(&mut self) {
        if let Some(h) = self.read_handle.take() {
            h.abort();
        }
        if let Some(h) = self.write_handle.take() {
            h.abort();
        }
    }
}
