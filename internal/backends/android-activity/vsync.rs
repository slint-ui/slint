// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore ALOOPER Condvar wakeups

//! Drives animation frames from the display's vsync using the Android Choreographer.
//!
//! The Choreographer delivers its callback through the `ALooper` it is fetched on, and
//! `ALooper_pollOnce` reports that as `ALOOPER_POLL_CALLBACK`, which the android-activity
//! `poll_events` wrapper logs as a spurious error. To keep the Choreographer off the main
//! looper, a dedicated helper thread owns its own looper, waits for each vsync, and wakes
//! the main event loop through its `AndroidAppWaker`. The main loop then advances the
//! animations and renders. Pacing therefore follows the actual refresh rate instead of a
//! fixed poll timeout.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crate::android_activity::AndroidAppWaker;
use crate::ndk::looper::{ForeignLooper, ThreadLooper};

/// The Choreographer is not wrapped by the `ndk` crate. Available since API 24; Slint's
/// Android minimum is API 26, so the symbols are always present.
mod ffi {
    use core::ffi::{c_long, c_void};

    #[repr(C)]
    pub struct AChoreographer {
        _unused: [u8; 0],
    }
    pub type FrameCallback = unsafe extern "C" fn(frame_time_nanos: c_long, data: *mut c_void);

    #[link(name = "android")]
    unsafe extern "C" {
        pub fn AChoreographer_getInstance() -> *mut AChoreographer;
        pub fn AChoreographer_postFrameCallback(
            choreographer: *mut AChoreographer,
            callback: FrameCallback,
            data: *mut c_void,
        );
    }
}

struct Shared {
    state: Mutex<State>,
    cond: Condvar,
    /// Set once the helper thread has a working Choreographer and can wake the main
    /// loop at every vsync. Until then (and if the thread never starts) the main loop
    /// keeps its own periodic wakeup so animations still advance.
    driving: AtomicBool,
    /// The helper thread's looper, published so the main thread can wake it on shutdown.
    looper: Mutex<Option<ForeignLooper>>,
    /// Wakes the main event loop when a vsync arrives.
    main_waker: AndroidAppWaker,
}

#[derive(Default)]
struct State {
    animating: bool,
    quit: bool,
}

/// Wakes the main event loop at each display vsync while animations are running.
pub struct VsyncDriver {
    shared: Arc<Shared>,
    handle: Option<JoinHandle<()>>,
}

impl VsyncDriver {
    pub fn new(main_waker: AndroidAppWaker) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            cond: Condvar::new(),
            driving: AtomicBool::new(false),
            looper: Mutex::new(None),
            main_waker,
        });
        let handle = std::thread::Builder::new()
            .name("slint vsync".into())
            .spawn({
                let shared = shared.clone();
                move || vsync_thread(shared)
            })
            .ok();
        Self { shared, handle }
    }

    /// Whether the helper thread is driving frames from vsync. When it is not, the
    /// caller must keep waking itself so that animations still advance.
    pub fn is_driving(&self) -> bool {
        self.shared.driving.load(Ordering::Acquire)
    }

    /// Publishes whether the main loop currently has active animations. Turning it on
    /// starts the per-vsync wakeups; turning it off parks the helper after the next frame.
    pub fn set_animating(&self, animating: bool) {
        let mut state = self.shared.state.lock().unwrap();
        if state.animating != animating {
            state.animating = animating;
            drop(state);
            self.shared.cond.notify_one();
        }
    }
}

impl Drop for VsyncDriver {
    fn drop(&mut self) {
        self.shared.state.lock().unwrap().quit = true;
        self.shared.cond.notify_one();
        // Interrupt a poll that may otherwise block until the next vsync, or forever
        // while the app is in the background.
        if let Some(looper) = self.shared.looper.lock().unwrap().as_ref() {
            looper.wake();
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn vsync_thread(shared: Arc<Shared>) {
    let looper = ThreadLooper::prepare();
    *shared.looper.lock().unwrap() = Some(looper.as_foreign().clone());

    // SAFETY: `AChoreographer_getInstance` needs a looper on the calling thread, which
    // `ThreadLooper::prepare` just established.
    let choreographer = unsafe { ffi::AChoreographer_getInstance() };
    if choreographer.is_null() {
        return;
    }
    shared.driving.store(true, Ordering::Release);

    loop {
        {
            let mut state = shared.state.lock().unwrap();
            while !state.animating && !state.quit {
                state = shared.cond.wait(state).unwrap();
            }
            if state.quit {
                break;
            }
        }

        // Request one frame and block until the Choreographer delivers it, or the main
        // thread wakes the looper to shut us down. The callback does nothing; the vsync
        // signal is simply that the poll returned.
        // SAFETY: `choreographer` belongs to this thread and stays valid.
        unsafe {
            ffi::AChoreographer_postFrameCallback(
                choreographer,
                noop_frame_callback,
                std::ptr::null_mut(),
            );
        }
        let _ = looper.poll_once();

        // Wake the main loop to advance the animations and render this frame. A wake
        // that was really a shutdown request is harmless: the next iteration breaks at
        // the parked wait above.
        shared.main_waker.wake();
    }
}

extern "C" fn noop_frame_callback(
    _frame_time_nanos: core::ffi::c_long,
    _data: *mut core::ffi::c_void,
) {
}
