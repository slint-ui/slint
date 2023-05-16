// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

/*!
    Support for timers.

    Timers are just a bunch of callbacks sorted by expiry date.
*/

#![warn(missing_docs)]
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use crate::animations::Instant;

type TimerCallback = Box<dyn FnMut()>;
type SingleShotTimerCallback = Box<dyn FnOnce()>;

/// The TimerMode specifies what should happen after the timer fired.
///
/// Used by the [`Timer::start`] function.
#[derive(Copy, Clone)]
#[repr(C)]
#[non_exhaustive]
pub enum TimerMode {
    /// A SingleShot timer is fired only once.
    SingleShot,
    /// A Repeated timer is fired repeatedly until it is stopped or dropped.
    Repeated,
}

/// Timer is a handle to the timer system that allows triggering a callback to be called
/// after a specified period of time.
///
/// Use [`Timer::start()`] to create a timer that can repeat at frequent interval, or
/// [`Timer::single_shot`] if you just want to call a function with a delay and do not
/// need to be able to stop it.
///
/// The timer will automatically stop when dropped. You must keep the Timer object
/// around for as long as you want the timer to keep firing.
///
/// The timer can only be used in the thread that runs the Slint event loop.
/// They will not fire if used in another thread.
///
/// ## Example
/// ```rust,no_run
/// # i_slint_backend_testing::init();
/// use slint::{Timer, TimerMode};
/// let timer = Timer::default();
/// timer.start(TimerMode::Repeated, std::time::Duration::from_millis(200), move || {
///    println!("This will be printed every 200ms.");
/// });
/// // ... more initialization ...
/// slint::run_event_loop();
/// ```
#[derive(Default)]
pub struct Timer {
    id: Cell<Option<usize>>,
}

impl Timer {
    /// Starts the timer with the given mode and interval, in order for the callback to called when the
    /// timer fires. If the timer has been started previously and not fired yet, then it will be restarted.
    ///
    /// Arguments:
    /// * `mode`: The timer mode to apply, i.e. whether to repeatedly fire the timer or just once.
    /// * `interval`: The duration from now until when the timer should fire. And the period of that timer
    ///    for [`Repeated`](TimerMode::Repeated) timers.
    /// * `callback`: The function to call when the time has been reached or exceeded.
    pub fn start(
        &self,
        mode: TimerMode,
        interval: core::time::Duration,
        callback: impl FnMut() + 'static,
    ) {
        CURRENT_TIMERS.with(|timers| {
            let mut timers = timers.borrow_mut();
            let id = timers.start_or_restart_timer(
                self.id.get(),
                mode,
                interval,
                CallbackVariant::MultiFire(Box::new(callback)),
            );
            self.id.set(Some(id));
        })
    }

    /// Starts the timer with the duration, in order for the callback to called when the
    /// timer fires. It is fired only once and then deleted.
    ///
    /// Arguments:
    /// * `duration`: The duration from now until when the timer should fire.
    /// * `callback`: The function to call when the time has been reached or exceeded.
    ///
    /// ## Example
    /// ```rust
    /// # i_slint_backend_testing::init();
    /// use slint::Timer;
    /// Timer::single_shot(std::time::Duration::from_millis(200), move || {
    ///    println!("This will be printed after 200ms.");
    /// });
    /// ```
    pub fn single_shot(duration: core::time::Duration, callback: impl FnOnce() + 'static) {
        CURRENT_TIMERS.with(|timers| {
            let mut timers = timers.borrow_mut();
            let id = timers.start_or_restart_timer(
                None,
                TimerMode::SingleShot,
                duration,
                CallbackVariant::SingleShot(Box::new(callback)),
            );
            timers.timers[id].removed = true;
        })
    }

    /// Stops the previously started timer. Does nothing if the timer has never been started.
    pub fn stop(&self) {
        if let Some(id) = self.id.get() {
            CURRENT_TIMERS.with(|timers| {
                timers.borrow_mut().deactivate_timer(id);
            });
        }
    }

    /// Restarts the timer. If the timer was previously started by calling [`Self::start()`]
    /// with a duration and callback, then the time when the callback will be next invoked
    /// is re-calculated to be in the specified duration relative to when this function is called.
    ///
    /// Does nothing if the timer was never started.
    pub fn restart(&self) {
        if let Some(id) = self.id.get() {
            CURRENT_TIMERS.with(|timers| {
                timers.borrow_mut().deactivate_timer(id);
                timers.borrow_mut().activate_timer(id);
            });
        }
    }

    /// Returns true if the timer is running; false otherwise.
    pub fn running(&self) -> bool {
        self.id
            .get()
            .map(|timer_id| CURRENT_TIMERS.with(|timers| timers.borrow().timers[timer_id].running))
            .unwrap_or(false)
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(id) = self.id.get() {
            let _ = CURRENT_TIMERS.try_with(|timers| {
                timers.borrow_mut().remove_timer(id);
            });
        }
    }
}

enum CallbackVariant {
    Empty,
    MultiFire(TimerCallback),
    SingleShot(SingleShotTimerCallback),
}

impl CallbackVariant {
    fn invoke(&mut self) {
        use CallbackVariant::*;
        match self {
            Empty => (),
            MultiFire(cb) => cb(),
            SingleShot(_) => {
                if let SingleShot(cb) = core::mem::replace(self, Empty) {
                    cb();
                }
            }
        }
    }
}

struct TimerData {
    duration: core::time::Duration,
    mode: TimerMode,
    running: bool,
    /// Set to true when it is removed when the callback is still running
    removed: bool,
    callback: CallbackVariant,
}

#[derive(Clone, Copy)]
struct ActiveTimer {
    id: usize,
    timeout: Instant,
}

/// TimerList provides the interface to the event loop for activating times and
/// determining the nearest timeout.
#[derive(Default)]
pub struct TimerList {
    timers: slab::Slab<TimerData>,
    active_timers: Vec<ActiveTimer>,
    /// If a callback is currently running, this is the id of the currently running callback
    callback_active: Option<usize>,
}

impl TimerList {
    /// Returns the timeout of the timer that should fire the soonest, or None if there
    /// is no timer active.
    pub fn next_timeout() -> Option<Instant> {
        CURRENT_TIMERS.with(|timers| {
            timers
                .borrow()
                .active_timers
                .first()
                .map(|first_active_timer| first_active_timer.timeout)
        })
    }

    /// Activates any expired timers by calling their callback function. Returns true if any timers were
    /// activated; false otherwise.
    pub fn maybe_activate_timers(now: Instant) -> bool {
        // Shortcut: Is there any timer worth activating?
        if TimerList::next_timeout().map(|timeout| now < timeout).unwrap_or(false) {
            return false;
        }

        CURRENT_TIMERS.with(|timers| {
            assert!(timers.borrow().callback_active.is_none(), "Recursion in timer code");

            let mut any_activated = false;

            // The active timer list is cleared here and not-yet-fired ones are inserted below, in order to allow
            // timer callbacks to register their own timers.
            let timers_to_process = core::mem::take(&mut timers.borrow_mut().active_timers);
            for active_timer in timers_to_process.into_iter() {
                if active_timer.timeout <= now {
                    any_activated = true;

                    let mut callback = {
                        let mut timers = timers.borrow_mut();

                        timers.callback_active = Some(active_timer.id);

                        // do it before invoking the callback, in case the callback wants to stop or adjust its own timer
                        if matches!(timers.timers[active_timer.id].mode, TimerMode::Repeated) {
                            timers.activate_timer(active_timer.id);
                        }

                        // have to release the borrow on `timers` before invoking the callback,
                        // so here we temporarily move the callback out of its permanent place
                        core::mem::replace(
                            &mut timers.timers[active_timer.id].callback,
                            CallbackVariant::Empty,
                        )
                    };

                    callback.invoke();

                    let mut timers = timers.borrow_mut();

                    let callback_register = &mut timers.timers[active_timer.id].callback;

                    // only emplace back the callback if its permanent store is still Empty:
                    // if not, it means the invoked callback has restarted its own timer with a new callback
                    if matches!(callback_register, CallbackVariant::Empty) {
                        *callback_register = callback;
                    }

                    timers.callback_active = None;

                    if timers.timers[active_timer.id].removed {
                        timers.timers.remove(active_timer.id);
                    }
                } else {
                    timers.borrow_mut().register_active_timer(active_timer);
                }
            }

            any_activated
        })
    }

    fn start_or_restart_timer(
        &mut self,
        id: Option<usize>,
        mode: TimerMode,
        duration: core::time::Duration,
        callback: CallbackVariant,
    ) -> usize {
        let timer_data = TimerData { duration, mode, running: false, removed: false, callback };
        let inactive_timer_id = if let Some(id) = id {
            self.deactivate_timer(id);
            self.timers[id] = timer_data;
            id
        } else {
            self.timers.insert(timer_data)
        };
        self.activate_timer(inactive_timer_id);
        inactive_timer_id
    }

    fn deactivate_timer(&mut self, id: usize) {
        let mut i = 0;
        while i < self.active_timers.len() {
            if self.active_timers[i].id == id {
                self.active_timers.remove(i);
                self.timers[id].running = false;
                break;
            } else {
                i += 1;
            }
        }
    }

    fn activate_timer(&mut self, timer_id: usize) {
        self.register_active_timer(ActiveTimer {
            id: timer_id,
            timeout: Instant::now() + self.timers[timer_id].duration,
        });
    }

    fn register_active_timer(&mut self, new_active_timer: ActiveTimer) {
        let insertion_index = lower_bound(&self.active_timers, |existing_timer| {
            existing_timer.timeout < new_active_timer.timeout
        });

        self.active_timers.insert(insertion_index, new_active_timer);
        self.timers[new_active_timer.id].running = true;
    }

    fn remove_timer(&mut self, timer_id: usize) {
        self.deactivate_timer(timer_id);
        if self.callback_active == Some(timer_id) {
            self.timers[timer_id].removed = true;
        } else {
            self.timers.remove(timer_id);
        }
    }
}

#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
use crate::unsafe_single_threaded::thread_local;

thread_local!(static CURRENT_TIMERS : RefCell<TimerList> = RefCell::default());

fn lower_bound<T>(vec: &[T], mut less_than: impl FnMut(&T) -> bool) -> usize {
    let mut left = 0;
    let mut right = vec.len();

    while left != right {
        let mid = left + (right - left) / 2;
        let value = &vec[mid];
        if less_than(value) {
            left = mid + 1;
        } else {
            right = mid;
        }
    }

    left
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;
    #[allow(non_camel_case_types)]
    type c_void = ();

    struct WrapFn {
        callback: extern "C" fn(*mut c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }

    impl Drop for WrapFn {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }

    impl WrapFn {
        fn call(&self) {
            (self.callback)(self.user_data)
        }
    }

    /// Start a timer with the given mode, duration in millisecond and callback. A timer id may be provided (first argument).
    /// A value of -1 for the timer id means a new timer is to be allocated.
    /// The (new) timer id is returned.
    /// The timer MUST be destroyed with slint_timer_destroy.
    #[no_mangle]
    pub extern "C" fn slint_timer_start(
        id: i64,
        mode: TimerMode,
        duration: u64,
        callback: extern "C" fn(*mut c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) -> i64 {
        let wrap = WrapFn { callback, user_data, drop_user_data };
        let timer = Timer::default();
        if id != -1 {
            timer.id.set(Some(id as _));
        }
        timer.start(mode, core::time::Duration::from_millis(duration), move || wrap.call());
        timer.id.take().map(|x| x as i64).unwrap_or(-1)
    }

    /// Execute a callback with a delay in millisecond
    #[no_mangle]
    pub extern "C" fn slint_timer_singleshot(
        delay: u64,
        callback: extern "C" fn(*mut c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) {
        let wrap = WrapFn { callback, user_data, drop_user_data };
        Timer::single_shot(core::time::Duration::from_millis(delay), move || wrap.call());
    }

    /// Stop a timer and free its raw data
    #[no_mangle]
    pub extern "C" fn slint_timer_destroy(id: i64) {
        if id == -1 {
            return;
        }
        let timer = Timer { id: Cell::new(Some(id as _)) };
        drop(timer);
    }

    /// Stop a timer
    #[no_mangle]
    pub extern "C" fn slint_timer_stop(id: i64) {
        if id == -1 {
            return;
        }
        let timer = Timer { id: Cell::new(Some(id as _)) };
        timer.stop();
        timer.id.take(); // Make sure that dropping the Timer doesn't unregister it. C++ will call destroy() in the destructor.
    }

    /// Restart a repeated timer
    #[no_mangle]
    pub extern "C" fn slint_timer_restart(id: i64) {
        if id == -1 {
            return;
        }
        let timer = Timer { id: Cell::new(Some(id as _)) };
        timer.restart();
        timer.id.take(); // Make sure that dropping the Timer doesn't unregister it. C++ will call destroy() in the destructor.
    }

    /// Returns true if the timer is running; false otherwise.
    #[no_mangle]
    pub extern "C" fn slint_timer_running(id: i64) -> bool {
        if id == -1 {
            return false;
        }
        let timer = Timer { id: Cell::new(Some(id as _)) };
        let running = timer.running();
        timer.id.take(); // Make sure that dropping the Timer doesn't unregister it. C++ will call destroy() in the destructor.
        running
    }
}

/**
```rust
i_slint_backend_testing::init();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::RefCell, time::Duration};
#[derive(Default)]
struct SharedState {
    timer_200: Timer,
    timer_200_called: usize,
    timer_500: Timer,
    timer_500_called: usize,
    timer_once: Timer,
    timer_once_called: usize,
}
let state = Rc::new(RefCell::new(SharedState::default()));
// Note: state will be leaked because of circular dependencies: don't do that in production
let state_ = state.clone();
state.borrow_mut().timer_200.start(TimerMode::Repeated, Duration::from_millis(200), move || {
    state_.borrow_mut().timer_200_called += 1;
});
let state_ = state.clone();
state.borrow_mut().timer_once.start(TimerMode::Repeated, Duration::from_millis(300), move || {
    state_.borrow_mut().timer_once_called += 1;
    state_.borrow().timer_once.stop();
});
let state_ = state.clone();
state.borrow_mut().timer_500.start(TimerMode::Repeated, Duration::from_millis(500), move || {
    state_.borrow_mut().timer_500_called += 1;
});
slint::platform::update_timers_and_animations();
i_slint_core::tests::slint_mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 0);
assert_eq!(state.borrow().timer_once_called, 0);
assert_eq!(state.borrow().timer_500_called, 0);
i_slint_core::tests::slint_mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 1);
assert_eq!(state.borrow().timer_once_called, 0);
assert_eq!(state.borrow().timer_500_called, 0);
i_slint_core::tests::slint_mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 1);
assert_eq!(state.borrow().timer_once_called, 1);
assert_eq!(state.borrow().timer_500_called, 0);
i_slint_core::tests::slint_mock_elapsed_time(200); // total: 500
assert_eq!(state.borrow().timer_200_called, 2);
assert_eq!(state.borrow().timer_once_called, 1);
assert_eq!(state.borrow().timer_500_called, 1);
for _ in 0..10 {
    i_slint_core::tests::slint_mock_elapsed_time(100);
}
// total: 1500
assert_eq!(state.borrow().timer_200_called, 7);
assert_eq!(state.borrow().timer_once_called, 1);
assert_eq!(state.borrow().timer_500_called, 3);
state.borrow().timer_once.restart();
state.borrow().timer_200.restart();
state.borrow().timer_500.stop();
slint::platform::update_timers_and_animations();
i_slint_core::tests::slint_mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 7);
assert_eq!(state.borrow().timer_once_called, 1);
assert_eq!(state.borrow().timer_500_called, 3);
slint::platform::update_timers_and_animations();
i_slint_core::tests::slint_mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 8);
assert_eq!(state.borrow().timer_once_called, 1);
assert_eq!(state.borrow().timer_500_called, 3);
slint::platform::update_timers_and_animations();
i_slint_core::tests::slint_mock_elapsed_time(100);
assert_eq!(state.borrow().timer_200_called, 8);
assert_eq!(state.borrow().timer_once_called, 2);
assert_eq!(state.borrow().timer_500_called, 3);
slint::platform::update_timers_and_animations();
i_slint_core::tests::slint_mock_elapsed_time(1000);
slint::platform::update_timers_and_animations();
slint::platform::update_timers_and_animations();
// Despite 1000ms have passed, the 200 timer is only called once because we didn't call update_timers_and_animations in between
assert_eq!(state.borrow().timer_200_called, 9);
assert_eq!(state.borrow().timer_once_called, 2);
assert_eq!(state.borrow().timer_500_called, 3);
let state_ = state.clone();
state.borrow().timer_200.start(TimerMode::SingleShot, Duration::from_millis(200), move || {
    state_.borrow_mut().timer_200_called += 1;
});
for _ in 0..5 {
    i_slint_core::tests::slint_mock_elapsed_time(75);
}
assert_eq!(state.borrow().timer_200_called, 10);
assert_eq!(state.borrow().timer_once_called, 2);
assert_eq!(state.borrow().timer_500_called, 3);
state.borrow().timer_200.restart();
for _ in 0..5 {
    i_slint_core::tests::slint_mock_elapsed_time(75);
}
assert_eq!(state.borrow().timer_200_called, 11);
assert_eq!(state.borrow().timer_once_called, 2);
assert_eq!(state.borrow().timer_500_called, 3);

// Test re-starting from a callback
let state_ = state.clone();
state.borrow_mut().timer_500.start(TimerMode::Repeated, Duration::from_millis(500), move || {
    state_.borrow_mut().timer_500_called += 1;
    let state__ = state_.clone();
    state_.borrow_mut().timer_500.start(TimerMode::Repeated, Duration::from_millis(500), move || {
        state__.borrow_mut().timer_500_called += 1000;
    });
    let state__ = state_.clone();
    state_.borrow_mut().timer_200.start(TimerMode::Repeated, Duration::from_millis(200), move || {
        state__.borrow_mut().timer_200_called += 1000;
    });
});
for _ in 0..20 {
    i_slint_core::tests::slint_mock_elapsed_time(100);
}
assert_eq!(state.borrow().timer_200_called, 7011);
assert_eq!(state.borrow().timer_once_called, 2);
assert_eq!(state.borrow().timer_500_called, 3004);
```
 */
#[cfg(doctest)]
const _TIMER_TESTS: () = ();
