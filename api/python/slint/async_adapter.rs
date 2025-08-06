// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use pyo3::prelude::*;
use pyo3_stub_gen::{derive::gen_stub_pyclass, derive::gen_stub_pymethods};

#[cfg(unix)]
struct PyFdWrapper(std::os::fd::RawFd);

#[cfg(unix)]
impl std::os::fd::AsFd for PyFdWrapper {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        unsafe { std::os::fd::BorrowedFd::borrow_raw(self.0) }
    }
}

#[cfg(windows)]
struct PyFdWrapper(#[cfg(windows)] std::os::windows::io::RawSocket);

#[cfg(windows)]
impl std::os::windows::io::AsSocket for PyFdWrapper {
    fn as_socket(&self) -> std::os::windows::io::BorrowedSocket<'_> {
        unsafe { std::os::windows::io::BorrowedSocket::borrow_raw(self.0) }
    }
}

struct AdapterInner {
    adapter: smol::Async<PyFdWrapper>,
    readable_callback: Option<Py<PyAny>>,
    writable_callback: Option<Py<PyAny>>,
}

#[gen_stub_pyclass]
#[pyclass(unsendable)]
pub struct AsyncAdapter {
    inner: Option<Rc<AdapterInner>>,
    task: Option<slint_interpreter::JoinHandle<()>>,
}

#[gen_stub_pymethods]
#[pymethods]
impl AsyncAdapter {
    #[new]
    fn py_new(fd: i32) -> Self {
        #[cfg(windows)]
        let fd = u64::try_from(fd).unwrap();
        AsyncAdapter {
            inner: Some(Rc::new(AdapterInner {
                adapter: smol::Async::new(PyFdWrapper(fd)).unwrap(),
                readable_callback: Default::default(),
                writable_callback: Default::default(),
            })),
            task: None,
        }
    }

    fn wait_for_readable(&mut self, callback: Py<PyAny>) {
        self.restart_after_mut_inner_access(|inner| {
            inner.readable_callback.replace(callback);
        });
    }

    fn wait_for_writable(&mut self, callback: Py<PyAny>) {
        self.restart_after_mut_inner_access(|inner| {
            inner.writable_callback.replace(callback);
        });
    }
}

impl AsyncAdapter {
    fn restart_after_mut_inner_access(&mut self, callback: impl FnOnce(&mut AdapterInner)) {
        if let Some(task) = self.task.take() {
            task.abort();
        }

        // This detaches and basically makes any existing future that might get woke up fail when
        // trying to upgrade the weak.
        let mut inner = Rc::into_inner(self.inner.take().unwrap()).unwrap();

        callback(&mut inner);

        let inner = Rc::new(inner);
        let inner_weak = Rc::downgrade(&inner);
        self.inner = Some(inner);
        self.task = Some(
            slint_interpreter::spawn_local(std::future::poll_fn(move |cx| loop {
                let Some(inner) = inner_weak.upgrade() else {
                    return std::task::Poll::Ready(());
                };

                let readable_poll_status: Option<std::task::Poll<Py<PyAny>>> =
                    inner.readable_callback.as_ref().map(|callback| {
                        if inner.adapter.poll_readable(cx).is_ready() {
                            std::task::Poll::Ready(Python::attach(|py| callback.clone_ref(py)))
                        } else {
                            std::task::Poll::Pending
                        }
                    });

                let writable_poll_status: Option<std::task::Poll<Py<PyAny>>> =
                    inner.writable_callback.as_ref().map(|callback| {
                        if inner.adapter.poll_writable(cx).is_ready() {
                            std::task::Poll::Ready(Python::attach(|py| callback.clone_ref(py)))
                        } else {
                            std::task::Poll::Pending
                        }
                    });

                let fd = inner.adapter.get_ref().0;

                drop(inner);

                if let Some(std::task::Poll::Ready(callback)) = &readable_poll_status {
                    Python::attach(|py| {
                        callback.call1(py, (fd,)).expect(
                            "unexpected failure running python async readable adapter callback",
                        );
                    });
                }

                if let Some(std::task::Poll::Ready(callback)) = &writable_poll_status {
                    Python::attach(|py| {
                        callback.call1(py, (fd,)).expect(
                            "unexpected failure running python async writable adapter callback",
                        );
                    });
                }

                match &readable_poll_status {
                    Some(std::task::Poll::Ready(..)) => continue, // poll again and then probably return in the next iteration
                    Some(std::task::Poll::Pending) => return std::task::Poll::Pending, // waker registered, come back later
                    None => {} // Nothing to poll
                }

                match &writable_poll_status {
                    Some(std::task::Poll::Ready(..)) => continue, // poll again and then probably return in the next iteration
                    Some(std::task::Poll::Pending) => return std::task::Poll::Pending, // waker registered, come back later
                    None => {} // Nothing to poll
                }

                return std::task::Poll::Ready(());
            }))
            .unwrap(),
        );
    }
}
