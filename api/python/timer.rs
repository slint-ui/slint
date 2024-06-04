// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;

#[derive(Copy, Clone)]
#[pyclass(name = "TimerMode")]
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

#[pyclass(name = "Timer", unsendable)]
pub struct PyTimer {
    timer: i_slint_core::timers::Timer,
}

#[pymethods]
impl PyTimer {
    #[new]
    fn py_new() -> Self {
        PyTimer { timer: Default::default() }
    }

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

    fn stop(&self) {
        self.timer.stop();
    }

    fn restart(&self) {
        self.timer.restart();
    }

    fn running(&self) -> bool {
        self.timer.running()
    }

    fn set_interval(&self, interval: chrono::Duration) -> PyResult<()> {
        let interval = interval
            .to_std()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        self.timer.set_interval(interval);
        Ok(())
    }
}
