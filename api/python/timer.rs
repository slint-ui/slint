// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use pyo3_stub_gen::{
    derive::gen_stub_pyclass, derive::gen_stub_pyclass_enum, derive::gen_stub_pymethods,
};

/// The TimerMode specifies what should happen after the timer fired.
///
/// Used by the `Timer.start()` function.
#[derive(Copy, Clone, PartialEq)]
#[gen_stub_pyclass_enum]
#[pyclass(name = "TimerMode", eq, eq_int)]
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
/// [`Timer::single_shot`] to trigger a callback only once.
///
/// The timer will automatically stop when garbage collected. You must keep the Timer object
/// around for as long as you want the timer to keep firing.
///
/// Timers can only be used in the thread that runs the Slint event loop. They don't
/// fire if used in another thread.
#[gen_stub_pyclass]
#[pyclass(name = "Timer", unsendable)]
pub struct PyTimer {
    timer: i_slint_core::timers::Timer,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyTimer {
    #[new]
    fn py_new() -> Self {
        PyTimer { timer: Default::default() }
    }

    /// Starts the timer with the given mode and interval, in order for the callback to called when the
    /// timer fires. If the timer has been started previously and not fired yet, then it will be restarted.
    ///
    /// Arguments:
    /// * `mode`: The timer mode to apply, i.e. whether to repeatedly fire the timer or just once.
    /// * `interval`: The duration from now until when the timer should firethe first time, and subsequently
    ///    for `TimerMode.Repeated` timers.
    /// * `callback`: The function to call when the time has been reached or exceeded.
    fn start(
        &self,
        mode: PyTimerMode,
        interval: chrono::Duration,
        callback: PyObject,
    ) -> PyResult<()> {
        let interval = interval
            .to_std()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        self.timer.start(mode.into(), interval, move || {
            Python::with_gil(|py| {
                callback.call0(py).expect("unexpected failure running python timer callback");
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
    fn single_shot(duration: chrono::Duration, callback: PyObject) -> PyResult<()> {
        let duration = duration
            .to_std()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        i_slint_core::timers::Timer::single_shot(duration, move || {
            Python::with_gil(|py| {
                callback.call0(py).expect("unexpected failure running python timer callback");
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
}
