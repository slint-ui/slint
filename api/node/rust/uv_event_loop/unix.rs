// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore nonblocking

//! Unix watcher: an async-io future on the Slint event loop that
//! watches libuv's backend fd (epoll/kqueue) for readability.

use super::uv;
use std::cell::Cell;
use std::os::fd::BorrowedFd;
use std::rc::Rc;

/// Borrows libuv's backend fd without closing it on drop.
struct FdWrapper(std::os::raw::c_int);

impl std::os::fd::AsFd for FdWrapper {
    fn as_fd(&self) -> BorrowedFd<'_> {
        // SAFETY: libuv owns this fd for the process lifetime.
        unsafe { BorrowedFd::borrow_raw(self.0) }
    }
}

/// Signals when libuv I/O arrives while winit blocks inside the
/// prepare callback.
#[derive(Clone)]
pub(super) struct Watcher {
    ready: Rc<Cell<bool>>,
}

impl Watcher {
    /// Spawn a future that watches the libuv backend fd and sets the
    /// ready flag when I/O arrives.
    pub(super) fn new(uv: &uv::Functions) -> napi::Result<Self> {
        // new_nonblocking: ioctl to set non-blocking fails on macOS kqueue fds.
        let async_fd =
            async_io::Async::new_nonblocking(FdWrapper(uv.backend_fd())).map_err(|e| {
                napi::Error::from_reason(format!("failed to create async fd watcher: {e}"))
            })?;

        let ready = Rc::new(Cell::new(false));
        let ready_for_future = ready.clone();
        slint_interpreter::spawn_local(async move {
            loop {
                if async_fd.readable().await.is_err() {
                    break;
                }
                ready_for_future.set(true);
            }
        })
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

        Ok(Self { ready })
    }

    /// Called before blocking in `process_events`.
    /// The fd watcher is permanently armed; nothing to do.
    pub(super) fn arm(&self, _uv_timeout_ms: std::os::raw::c_int) {}

    /// Take and reset the I/O-ready flag.
    pub(super) fn take_ready(&self) -> bool {
        self.ready.replace(false)
    }
}
