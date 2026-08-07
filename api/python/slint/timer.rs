// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore timedelta
use std::cell::RefCell;
use std::rc::Rc;

use pyo3::prelude::*;
use pyo3::{PyTraverseError, gc::PyVisit};

/// The TimerMode specifies what should happen after the timer fired.
///
/// Used by the `Timer.start()` function.
#[derive(Copy, Clone, PartialEq)]
#[pyclass(name = "TimerMode", eq, eq_int, from_py_object)]
pub enum PyTimerMode {
    /// A SingleShot timer is fired only once.
    SingleShot,
    /// A Repeated timer is fired repeatedly until it is stopped or dropped.
    Repeated,
}

impl From<PyTimerMode> for i_slint_core::timers::TimerMode {
    fn from(value: PyTimerMode) -> Self {
        match value {
            PyTimerMode::SingleShot => i_slint_core::timers::TimerMode::SingleShot,
            PyTimerMode::Repeated => i_slint_core::timers::TimerMode::Repeated,
        }
    }
}

/// Timer is a handle to the timer system that triggers a callback after a specified
/// period of time.
///
/// Use `Timer.start()` to create a timer that that repeatedly triggers a callback, or
/// `Timer.single_shot()` to trigger a callback only once.
///
/// The timer will automatically stop when garbage collected. You must keep the Timer object
/// around for as long as you want the timer to keep firing.
///
/// ```python
/// class AppWindow(...)
///     def __init__(self):
///         super().__init__()
///         self.my_timer = None
///
///     @slint.callback
///     def button_clicked(self):
///         self.my_timer = slint.Timer()
///         self.my_timer.start(timedelta(seconds=1), self.do_something)
///
///     def do_something(self):
///         pass
/// ```
///
/// Timers can only be used in the thread that runs the Slint event loop. They don't
/// fire if used in another thread.
#[pyclass(name = "Timer", unsendable)]
pub struct PyTimer {
    timer: i_slint_core::timers::Timer,
    /// Shared with the closure i-slint-core keeps in its (GC-invisible) timer list, so
    /// that `__clear__` releases the core closure's reference too, not just ours.
    callback: Rc<RefCell<Option<Py<PyAny>>>>,
}

#[pymethods]
impl PyTimer {
    #[new]
    fn py_new() -> Self {
        PyTimer { timer: Default::default(), callback: Default::default() }
    }

    /// Starts the timer with the given mode and interval, in order for the callback to called when the
    /// timer fires. If the timer has been started previously and not fired yet, then it will be restarted.
    ///
    /// Arguments:
    /// * `mode`: The timer mode to apply, i.e. whether to repeatedly fire the timer or just once.
    /// * `interval`: The duration from now until when the timer should fire the first time, and subsequently
    ///    for `TimerMode.Repeated` timers.
    /// * `callback`: The function to call when the time has been reached or exceeded.
    fn start(
        &self,
        mode: PyTimerMode,
        interval: chrono::Duration,
        callback: Py<PyAny>,
    ) -> PyResult<()> {
        let interval = interval
            .to_std()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        // Bind the old callback rather than discarding it in place: releasing it may run
        // Python code that touches this timer, and a binding keeps that until after the
        // borrow ends. (`let _ =` would drop it while the borrow is still held.)
        let previous = self.callback.borrow_mut().replace(callback);
        drop(previous);

        let slot = self.callback.clone();
        self.timer.start(mode.into(), interval, move || {
            Python::attach(|py| {
                // Take a strong reference and release the borrow before calling into
                // Python: the callback may start or stop this very timer.
                let Some(callback) = slot.borrow().as_ref().map(|cb| cb.clone_ref(py)) else {
                    // Cleared by `__clear__` while the timer was still armed.
                    return;
                };
                if let Err(err) = callback.call0(py) {
                    crate::handle_unraisable(
                        py,
                        "unexpected failure running python timer callback".into(),
                        err,
                    );
                }
            });
        });
        Ok(())
    }

    /// Starts the timer with the duration and the callback to called when the
    /// timer fires. It is fired only once and then deleted.
    ///
    /// Arguments:
    /// * `duration`: The duration from now until when the timer should fire.
    /// * `callback`: The function to call when the time has been reached or exceeded.
    #[staticmethod]
    fn single_shot(duration: chrono::Duration, callback: Py<PyAny>) -> PyResult<()> {
        let duration = duration
            .to_std()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        i_slint_core::timers::Timer::single_shot(duration, move || {
            Python::attach(|py| {
                if let Err(err) = callback.call0(py) {
                    crate::handle_unraisable(
                        py,
                        "unexpected failure running python singleshot timer callback".into(),
                        err,
                    );
                }
            });
        });
        Ok(())
    }

    /// Stops the previously started timer. Does nothing if the timer has never been started.
    fn stop(&self) {
        self.timer.stop();
    }

    /// Restarts the timer. If the timer was previously started by calling `Timer.start()`
    /// with a duration and callback, then the time when the callback will be next invoked
    /// is re-calculated to be in the specified duration relative to when this function is called.
    ///
    /// Does nothing if the timer was never started.
    fn restart(&self) {
        self.timer.restart();
    }

    /// Set to true if the timer is running; false otherwise.
    #[getter]
    fn running(&self) -> bool {
        self.timer.running()
    }

    /// The duration of timer.
    ///
    /// When setting this property and the timer is running (see `Timer.running`),
    /// then the time when the callback will be next invoked is re-calculated to be in the
    /// specified duration relative to when this property is set.
    #[setter]
    fn set_interval(&self, interval: chrono::Duration) -> PyResult<()> {
        let interval = interval
            .to_std()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        self.timer.set_interval(interval);
        Ok(())
    }

    #[getter]
    fn interval(&self) -> core::time::Duration {
        self.timer.interval()
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        // i-slint-core stores the callback inside a boxed closure in a thread local, where
        // Python's cyclic GC can't see it. Report it here, so that the common
        // `self.timer.start(..., self.on_tick)` pattern - a cycle through the timer - stays
        // collectable.
        if let Ok(slot) = self.callback.try_borrow()
            && let Some(callback) = slot.as_ref()
        {
            visit.call(callback)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        // Stop first: the closure in the core timer list outlives this call and would
        // otherwise keep firing as a no-op until the timer is dropped.
        self.timer.stop();
        // `and_then` drops the borrow before yielding the callback, so releasing it here
        // is already outside the borrow.
        let _ = self.callback.try_borrow_mut().ok().and_then(|mut slot| slot.take());
    }
}
