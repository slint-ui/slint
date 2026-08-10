// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Windows watcher: waits on libuv's I/O completion port from a
//! dedicated thread.
//!
//! libuv has no accessor for the IOCP (`uv_backend_fd()` returns -1 on
//! Windows), but the handle sits at a fixed offset in `uv_loop_s`: the
//! struct's public prefix is ABI-frozen in uv.h and `iocp` is the first
//! Windows-private field. [`find_iocp`] reads the handle at that offset
//! and verifies with the OS that it really is an I/O completion port;
//! on any mismatch the caller falls back to JS-side polling.
//!
//! Electron integrates the loops with the same technique
//! (`shell/common/node_bindings_win.cc`), except it takes the offset
//! from uv.h at compile time.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender};
use windows::Wdk::Foundation::{NtQueryObject, ObjectTypeInformation};
use windows::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::System::IO::{
    GetQueuedCompletionStatus, OVERLAPPED, PostQueuedCompletionStatus,
};
use windows::Win32::System::WindowsProgramming::PUBLIC_OBJECT_TYPE_INFORMATION;

/// Byte offset of the `iocp` field in `uv_loop_s` for the given libuv
/// `UV_VERSION_HEX`, or `None` when the layout is unknown.
fn iocp_offset(uv_version: u32) -> Option<usize> {
    const V1_38_0: u32 = 0x01_2600;
    if uv_version < V1_38_0 {
        return None;
    }
    // Only architectures with a verified layout; the rest fall back to polling.
    if cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
        Some(56)
    } else if cfg!(target_arch = "x86") {
        Some(28)
    } else {
        None
    }
}

/// Locate libuv's I/O completion port inside the loop struct.
/// Returns the raw handle value, or the reason it wasn't found.
pub(super) fn find_iocp(
    uv_loop: *mut napi::sys::uv_loop_s,
    uv_version: u32,
    uv_loop_size: usize,
) -> Result<usize, &'static str> {
    let offset = iocp_offset(uv_version).ok_or("libuv version too old")?;
    if offset + size_of::<usize>() > uv_loop_size {
        return Err("uv_loop_t is smaller than the expected iocp offset");
    }

    // SAFETY: in-bounds read of the live, pointer-aligned loop struct.
    let handle = unsafe { (uv_loop as *const u8).add(offset).cast::<usize>().read() };
    let iocp = HANDLE(handle as *mut core::ffi::c_void);
    if handle == 0 || iocp == INVALID_HANDLE_VALUE {
        return Err("no handle at the expected iocp offset");
    }
    if !is_io_completion_port(iocp) {
        return Err("the value at the expected iocp offset is not an I/O completion port");
    }
    Ok(handle)
}

/// Ask the kernel for the NT object type of `handle`.
/// An invalid handle value just fails the query; no crash path.
fn is_io_completion_port(handle: HANDLE) -> bool {
    // Fixed-size struct followed by the (short) type name; u64 for alignment.
    let mut buf = [0u64; 128];
    let status = unsafe {
        NtQueryObject(
            Some(handle),
            ObjectTypeInformation,
            Some(buf.as_mut_ptr().cast()),
            size_of_val(&buf) as u32,
            None,
        )
    };
    if status.is_err() {
        return false;
    }
    // SAFETY: NtQueryObject filled the buffer with this layout.
    let info = unsafe { &*(buf.as_ptr() as *const PUBLIC_OBJECT_TYPE_INFORMATION) };
    if info.TypeName.Buffer.is_null() {
        return false;
    }
    // SAFETY: TypeName points into `buf`; Length is in bytes.
    let name = unsafe {
        std::slice::from_raw_parts(info.TypeName.Buffer.0, (info.TypeName.Length / 2) as usize)
    };
    name.iter().copied().eq("IoCompletion".encode_utf16())
}

/// Signals when libuv I/O arrives while winit blocks inside the
/// prepare callback.
///
/// A dedicated thread waits on the IOCP one arm at a time. Any packet
/// it takes out of the port is immediately posted back, so libuv still
/// processes it — a stolen packet only costs one extra loop iteration,
/// because the ready flag makes the prepare callback hand control
/// straight back to libuv's own poll.
#[derive(Clone)]
pub(super) struct Watcher {
    ready: Arc<AtomicBool>,
    arm_tx: SyncSender<u32>,
}

impl Watcher {
    /// Spawn the watcher thread for the loop's IOCP.
    /// The thread lives for the rest of the process, like the Unix
    /// watcher future.
    pub(super) fn new(uv: &super::uv::Functions) -> napi::Result<Self> {
        let iocp = uv.iocp();
        let ready = Arc::new(AtomicBool::new(false));
        let (arm_tx, arm_rx) = std::sync::mpsc::sync_channel(1);
        let thread_ready = ready.clone();
        std::thread::Builder::new()
            .name("slint-uv-iocp-watcher".into())
            .spawn(move || watcher_thread(iocp, arm_rx, thread_ready))
            .map_err(|e| {
                napi::Error::from_reason(format!("failed to spawn IOCP watcher thread: {e}"))
            })?;
        Ok(Self { ready, arm_tx })
    }

    /// Ask the watcher to wait for one completion packet, up to the
    /// libuv timer deadline. No-op when it is already waiting.
    pub(super) fn arm(&self, uv_timeout_ms: std::os::raw::c_int) {
        let timeout = if uv_timeout_ms < 0 { u32::MAX } else { uv_timeout_ms as u32 };
        let _ = self.arm_tx.try_send(timeout);
    }

    /// Take and reset the I/O-ready flag.
    pub(super) fn take_ready(&self) -> bool {
        self.ready.swap(false, Ordering::AcqRel)
    }
}

fn watcher_thread(iocp: usize, arm_rx: Receiver<u32>, ready: Arc<AtomicBool>) {
    let iocp = HANDLE(iocp as *mut core::ffi::c_void);
    for timeout in arm_rx {
        let mut bytes = 0u32;
        let mut key = 0usize;
        let mut overlapped: *mut OVERLAPPED = std::ptr::null_mut();
        let ok = unsafe {
            GetQueuedCompletionStatus(iocp, &mut bytes, &mut key, &mut overlapped, timeout)
        };
        // The call yields a packet if it succeeded, or if it failed
        // with a non-null OVERLAPPED (completion of a failed I/O).
        if ok.is_ok() || !overlapped.is_null() {
            // Put the packet back so libuv reads the I/O's final status on
            // the main thread; we only peeked to learn that work is ready.
            let over = (!overlapped.is_null()).then_some(overlapped as *const OVERLAPPED);
            let _ = unsafe { PostQueuedCompletionStatus(iocp, bytes, key, over) };
            ready.store(true, Ordering::Release);
            // Wake winit out of `process_events`; the prepare loop then
            // reads the ready flag.
            let _ = i_slint_core::api::invoke_from_event_loop(|| {});
        }
        // On timeout or port teardown, go dormant until re-armed. Timer
        // deadlines wake the main thread through its own timeout.
    }
}
