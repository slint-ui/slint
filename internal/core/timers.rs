// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore singleshot

/*!
    Support for timers.

    Timers are just a bunch of callbacks sorted by expiry date.
*/

#![warn(missing_docs)]
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
};

use crate::animations::Instant;

type TimerCallback = Box<dyn FnMut()>;
type SingleShotTimerCallback = Box<dyn FnOnce()>;

/// The TimerMode specifies what should happen after the timer fired.
///
/// Used by the [`Timer::start()`] function.
#[derive(Copy, Clone)]
#[repr(u8)]
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
/// # i_slint_backend_testing::init_no_event_loop();
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
    id: Cell<Option<NonZeroUsize>>,
    /// The timer cannot be moved between treads
    _phantom: core::marker::PhantomData<*mut ()>,
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
                self.id(),
                mode,
                interval,
                CallbackVariant::MultiFire(Box::new(callback)),
            );
            self.set_id(Some(id));
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
    /// # i_slint_backend_testing::init_no_event_loop();
    /// use slint::Timer;
    /// Timer::single_shot(std::time::Duration::from_millis(200), move || {
    ///    println!("This will be printed after 200ms.");
    /// });
    /// ```
    pub fn single_shot(duration: core::time::Duration, callback: impl FnOnce() + 'static) {
        CURRENT_TIMERS.with(|timers| {
            let mut timers = timers.borrow_mut();
            timers.start_or_restart_timer(
                None,
                TimerMode::SingleShot,
                duration,
                CallbackVariant::SingleShot(Box::new(callback)),
            );
        })
    }

    /// Stops the previously started timer. Does nothing if the timer has never been started.
    pub fn stop(&self) {
        if let Some(id) = self.id() {
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
        if let Some(id) = self.id() {
            CURRENT_TIMERS.with(|timers| {
                timers.borrow_mut().deactivate_timer(id);
                timers.borrow_mut().activate_timer(id);
            });
        }
    }

    /// Returns true if the timer is running; false otherwise.
    pub fn running(&self) -> bool {
        self.id()
            .map(|timer_id| CURRENT_TIMERS.with(|timers| timers.borrow().timers[timer_id].running))
            .unwrap_or(false)
    }

    /// Change the duration of timer. If the timer was previously started by calling [`Self::start()`]
    /// with a duration and callback, then the time when the callback will be next invoked
    /// is re-calculated to be in the specified duration relative to when this function is called.
    ///
    /// Does nothing if the timer was never started.
    ///
    /// Arguments:
    /// * `interval`: The duration from now until when the timer should fire. And the period of that timer
    ///    for [`Repeated`](TimerMode::Repeated) timers.
    pub fn set_interval(&self, interval: core::time::Duration) {
        if let Some(id) = self.id() {
            CURRENT_TIMERS.with(|timers| {
                timers.borrow_mut().set_interval(id, interval);
            });
        }
    }

    fn id(&self) -> Option<usize> {
        self.id.get().map(|v| usize::from(v) - 1)
    }

    fn set_id(&self, id: Option<usize>) {
        self.id.set(id.and_then(|v| NonZeroUsize::new(v + 1)));
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(id) = self.id() {
            let _ = CURRENT_TIMERS.try_with(|timers| {
                let callback = timers.borrow_mut().remove_timer(id);
                // drop the callback without having CURRENT_TIMERS borrowed
                drop(callback);
            });
        }
    }
}

enum CallbackVariant {
    Empty,
    MultiFire(TimerCallback),
    SingleShot(SingleShotTimerCallback),
}

struct TimerData {
    duration: core::time::Duration,
    mode: TimerMode,
    running: bool,
    /// Set to true when it is removed when the callback is still running
    removed: bool,
    /// true if it is in the cached the active_timers list in the maybe_activate_timers stack
    being_activated: bool,

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
            {
                let mut timers = timers.borrow_mut();
                for active_timer in &timers_to_process {
                    let timer = &mut timers.timers[active_timer.id];
                    assert!(!timer.being_activated);
                    timer.being_activated = true;
                }
            }
            for active_timer in timers_to_process.into_iter() {
                if active_timer.timeout <= now {
                    any_activated = true;

                    let mut callback = {
                        let mut timers = timers.borrow_mut();

                        timers.callback_active = Some(active_timer.id);

                        // do it before invoking the callback, in case the callback wants to stop or adjust its own timer
                        if matches!(timers.timers[active_timer.id].mode, TimerMode::Repeated) {
                            timers.activate_timer(active_timer.id);
                        } else {
                            timers.timers[active_timer.id].running = false;
                        }

                        // have to release the borrow on `timers` before invoking the callback,
                        // so here we temporarily move the callback out of its permanent place
                        core::mem::replace(
                            &mut timers.timers[active_timer.id].callback,
                            CallbackVariant::Empty,
                        )
                    };

                    match callback {
                        CallbackVariant::Empty => (),
                        CallbackVariant::MultiFire(ref mut cb) => cb(),
                        CallbackVariant::SingleShot(cb) => {
                            cb();
                            timers.borrow_mut().callback_active = None;
                            timers.borrow_mut().timers.remove(active_timer.id);
                            continue;
                        }
                    };

                    let mut timers = timers.borrow_mut();

                    let callback_register = &mut timers.timers[active_timer.id].callback;

                    // only emplace back the callback if its permanent store is still Empty:
                    // if not, it means the invoked callback has restarted its own timer with a new callback
                    if matches!(callback_register, CallbackVariant::Empty) {
                        *callback_register = callback;
                    }

                    timers.callback_active = None;
                    let t = &mut timers.timers[active_timer.id];
                    if t.removed {
                        timers.timers.remove(active_timer.id);
                    } else {
                        t.being_activated = false;
                    }
                } else {
                    let mut timers = timers.borrow_mut();
                    let t = &mut timers.timers[active_timer.id];
                    if t.removed {
                        timers.timers.remove(active_timer.id);
                    } else {
                        t.being_activated = false;
                        timers.register_active_timer(active_timer);
                    }
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
        let timer_data = TimerData {
            duration,
            mode,
            running: false,
            removed: false,
            callback,
            being_activated: false,
        };
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

    fn activate_timer(&mut self, id: usize) {
        self.register_active_timer(ActiveTimer {
            id,
            timeout: Instant::now() + self.timers[id].duration,
        });
    }

    fn register_active_timer(&mut self, new_active_timer: ActiveTimer) {
        let insertion_index = lower_bound(&self.active_timers, |existing_timer| {
            existing_timer.timeout < new_active_timer.timeout
        });

        self.active_timers.insert(insertion_index, new_active_timer);
        self.timers[new_active_timer.id].running = true;
    }

    fn remove_timer(&mut self, id: usize) -> CallbackVariant {
        self.deactivate_timer(id);
        let t = &mut self.timers[id];
        if t.being_activated {
            t.removed = true;
            CallbackVariant::Empty
        } else {
            self.timers.remove(id).callback
        }
    }

    fn set_interval(&mut self, id: usize, duration: core::time::Duration) {
        let timer = &self.timers[id];

        if !matches!(timer.callback, CallbackVariant::MultiFire { .. }) {
            return;
        }

        if timer.running {
            self.deactivate_timer(id);
            self.timers[id].duration = duration;
            self.activate_timer(id);
        } else {
            self.timers[id].duration = duration;
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
        id: usize,
        mode: TimerMode,
        duration: u64,
        callback: extern "C" fn(*mut c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) -> usize {
        let wrap = WrapFn { callback, user_data, drop_user_data };
        let timer = Timer::default();
        if id != 0 {
            timer.id.set(NonZeroUsize::new(id));
        }
        timer.start(mode, core::time::Duration::from_millis(duration), move || wrap.call());
        timer.id.take().map(|x| usize::from(x)).unwrap_or(0)
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
    pub extern "C" fn slint_timer_destroy(id: usize) {
        if id == 0 {
            return;
        }
        let timer = Timer { id: Cell::new(NonZeroUsize::new(id)), _phantom: Default::default() };
        drop(timer);
    }

    /// Stop a timer
    #[no_mangle]
    pub extern "C" fn slint_timer_stop(id: usize) {
        if id == 0 {
            return;
        }
        let timer = Timer { id: Cell::new(NonZeroUsize::new(id)), _phantom: Default::default() };
        timer.stop();
        timer.id.take(); // Make sure that dropping the Timer doesn't unregister it. C++ will call destroy() in the destructor.
    }

    /// Restart a repeated timer
    #[no_mangle]
    pub extern "C" fn slint_timer_restart(id: usize) {
        if id == 0 {
            return;
        }
        let timer = Timer { id: Cell::new(NonZeroUsize::new(id)), _phantom: Default::default() };
        timer.restart();
        timer.id.take(); // Make sure that dropping the Timer doesn't unregister it. C++ will call destroy() in the destructor.
    }

    /// Returns true if the timer is running; false otherwise.
    #[no_mangle]
    pub extern "C" fn slint_timer_running(id: usize) -> bool {
        if id == 0 {
            return false;
        }
        let timer = Timer { id: Cell::new(NonZeroUsize::new(id)), _phantom: Default::default() };
        let running = timer.running();
        timer.id.take(); // Make sure that dropping the Timer doesn't unregister it. C++ will call destroy() in the destructor.
        running
    }
}

/**
```rust
i_slint_backend_testing::init_no_event_loop();
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

// Test set interval
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

let state_ = state.clone();
slint::platform::update_timers_and_animations();
for _ in 0..5 {
    i_slint_core::tests::slint_mock_elapsed_time(100);
}
slint::platform::update_timers_and_animations();
assert_eq!(state.borrow().timer_200_called, 7013);
assert_eq!(state.borrow().timer_once_called, 3);
assert_eq!(state.borrow().timer_500_called, 3005);

for _ in 0..20 {
    state.borrow().timer_200.set_interval(Duration::from_millis(200 * 2));
    state.borrow().timer_once.set_interval(Duration::from_millis(300 * 2));
    state.borrow().timer_500.set_interval(Duration::from_millis(500 * 2));

    assert_eq!(state.borrow().timer_200_called, 7013);
    assert_eq!(state.borrow().timer_once_called, 3);
    assert_eq!(state.borrow().timer_500_called, 3005);

    i_slint_core::tests::slint_mock_elapsed_time(100);
}

slint::platform::update_timers_and_animations();
for _ in 0..9 {
    i_slint_core::tests::slint_mock_elapsed_time(100);
}
slint::platform::update_timers_and_animations();
assert_eq!(state.borrow().timer_200_called, 7015);
assert_eq!(state.borrow().timer_once_called, 3);
assert_eq!(state.borrow().timer_500_called, 3006);

state.borrow_mut().timer_once.restart();
for _ in 0..4 {
    i_slint_core::tests::slint_mock_elapsed_time(100);
}
assert_eq!(state.borrow().timer_once_called, 3);
for _ in 0..4 {
    i_slint_core::tests::slint_mock_elapsed_time(100);
}
assert_eq!(state.borrow().timer_once_called, 4);

```
 */
#[cfg(doctest)]
const _TIMER_TESTS: () = ();

/**
 * Test that deleting an active timer from a timer event works.
```rust
// There is a 200 ms timer that increase variable1
// after 500ms, that timer is destroyed by a single shot timer,
// and a new new timer  increase variable2
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::RefCell, time::Duration};
#[derive(Default)]
struct SharedState {
    repeated_timer: Timer,
    variable1: usize,
    variable2: usize,
}
let state = Rc::new(RefCell::new(SharedState::default()));
// Note: state will be leaked because of circular dependencies: don't do that in production
let state_ = state.clone();
state.borrow_mut().repeated_timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
    state_.borrow_mut().variable1 += 1;
});
let state_ = state.clone();
Timer::single_shot(Duration::from_millis(500), move || {
    state_.borrow_mut().repeated_timer = Default::default();
    let state = state_.clone();
    state_.borrow_mut().repeated_timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
        state.borrow_mut().variable2 += 1;
    })
} );
i_slint_core::tests::slint_mock_elapsed_time(10);
assert_eq!(state.borrow().variable1, 0);
assert_eq!(state.borrow().variable2, 0);
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 1);
assert_eq!(state.borrow().variable2, 0);
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 0);
i_slint_core::tests::slint_mock_elapsed_time(100);
// More than 500ms have elapsed, the single shot timer should have been activated, but that has no effect on variable 1 and 2
// This should just restart the timer so that the next change should happen 200ms from now
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 0);
i_slint_core::tests::slint_mock_elapsed_time(110);
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 0);
i_slint_core::tests::slint_mock_elapsed_time(100);
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 1);
i_slint_core::tests::slint_mock_elapsed_time(100);
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 1);
i_slint_core::tests::slint_mock_elapsed_time(100);
assert_eq!(state.borrow().variable1, 2);
assert_eq!(state.borrow().variable2, 2);
```
 */
#[cfg(doctest)]
const _BUG3029: () = ();

/**
 * Test that starting a singleshot timer works
```rust
// There is a 200 ms singleshot timer that increase variable1
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::RefCell, time::Duration};
#[derive(Default)]
struct SharedState {
    variable1: usize,
}
let state = Rc::new(RefCell::new(SharedState::default()));
// Note: state will be leaked because of circular dependencies: don't do that in production
let state_ = state.clone();
let timer = Timer::default();

timer.start(TimerMode::SingleShot, Duration::from_millis(200), move || {
    state_.borrow_mut().variable1 += 1;
});

// Singleshot timer set up and run...
assert!(timer.running());
i_slint_core::tests::slint_mock_elapsed_time(10);
assert!(timer.running());
assert_eq!(state.borrow().variable1, 0);
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 1);
assert!(!timer.running());
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 1); // It's singleshot, it only triggers once!
assert!(!timer.running());

// Restart a previously set up singleshot timer
timer.restart();
assert!(timer.running());
assert_eq!(state.borrow().variable1, 1);
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2);
assert!(!timer.running());
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2); // It's singleshot, it only triggers once!
assert!(!timer.running());

// Stop a non-running singleshot timer
timer.stop();
assert!(!timer.running());
assert_eq!(state.borrow().variable1, 2);
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2);
assert!(!timer.running());
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2); // It's singleshot, it only triggers once!
assert!(!timer.running());

// Stop a running singleshot timer
timer.restart();
assert!(timer.running());
assert_eq!(state.borrow().variable1, 2);
i_slint_core::tests::slint_mock_elapsed_time(10);
timer.stop();
assert!(!timer.running());
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2);
assert!(!timer.running());
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2); // It's singleshot, it only triggers once!
assert!(!timer.running());

// set_interval on a non-running singleshot timer
timer.set_interval(Duration::from_millis(300));
assert!(!timer.running());
i_slint_core::tests::slint_mock_elapsed_time(1000);
assert_eq!(state.borrow().variable1, 2);
assert!(!timer.running());
timer.restart();
assert!(timer.running());
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 2);
assert!(timer.running());
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 3);
assert!(!timer.running());
i_slint_core::tests::slint_mock_elapsed_time(300);
assert_eq!(state.borrow().variable1, 3); // It's singleshot, it only triggers once!
assert!(!timer.running());

// set_interval on a running singleshot timer
timer.restart();
assert!(timer.running());
assert_eq!(state.borrow().variable1, 3);
i_slint_core::tests::slint_mock_elapsed_time(290);
timer.set_interval(Duration::from_millis(400));
assert!(timer.running());
i_slint_core::tests::slint_mock_elapsed_time(200);
assert_eq!(state.borrow().variable1, 3);
assert!(timer.running());
i_slint_core::tests::slint_mock_elapsed_time(250);
assert_eq!(state.borrow().variable1, 4);
assert!(!timer.running());
i_slint_core::tests::slint_mock_elapsed_time(400);
assert_eq!(state.borrow().variable1, 4); // It's singleshot, it only triggers once!
assert!(!timer.running());
```
 */
#[cfg(doctest)]
const _SINGLESHOT_START: () = ();

/**
 * Test that it's possible to start a new timer from within Drop of a timer's closure.
 * This may happen when a timer's closure is dropped, that closure holds the last reference
 * to a component, that component is destroyed, and the accesskit code schedules a reload_tree
 * via a single shot.
```rust
i_slint_backend_testing::init_no_event_loop();
use slint::{Timer, TimerMode};
use std::{rc::Rc, cell::Cell, time::Duration};
#[derive(Default)]
struct CapturedInClosure {
    last_fired: Option<Rc<Cell<bool>>>,
}
impl Drop for CapturedInClosure {
    fn drop(&mut self) {
        if let Some(last_fired) = self.last_fired.as_ref().cloned() {
            Timer::single_shot(Duration::from_millis(100), move || last_fired.set(true));
        }
    }
}

let last_fired = Rc::new(Cell::new(false));

let mut cap_in_clos = CapturedInClosure::default();

let timer_to_stop = Timer::default();
timer_to_stop.start(TimerMode::Repeated, Duration::from_millis(100), {
    let last_fired = last_fired.clone();
    move || {
    cap_in_clos.last_fired = Some(last_fired.clone());
}});

assert_eq!(last_fired.get(), false);
i_slint_core::tests::slint_mock_elapsed_time(110);
assert_eq!(last_fired.get(), false);
drop(timer_to_stop);

i_slint_core::tests::slint_mock_elapsed_time(110);
assert_eq!(last_fired.get(), true);
```
 */
#[cfg(doctest)]
const _TIMER_CLOSURE_DROP_STARTS_NEW_TIMER: () = ();
