// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::Devices;

pub enum Timer {
    #[cfg(slint_debug_performance)]
    Running { start_time: core::time::Duration },
    #[cfg(slint_debug_performance)]
    Stopped { elapsed: core::time::Duration },
    #[cfg(not(debug_performance))]
    Noop,
}

impl Timer {
    pub fn new(_devices: &dyn Devices) -> Self {
        #[cfg(slint_debug_performance)]
        return Self::Running { start_time: _devices.time() };
        #[cfg(not(debug_performance))]
        return Self::Noop;
    }
    pub fn new_stopped() -> Self {
        #[cfg(slint_debug_performance)]
        return Self::Stopped { elapsed: core::time::Duration::new(0, 0) };
        #[cfg(not(debug_performance))]
        return Self::Noop;
    }

    #[cfg(slint_debug_performance)]
    pub fn elapsed(&self, _devices: &dyn Devices) -> core::time::Duration {
        match self {
            #[cfg(slint_debug_performance)]
            Self::Running { start_time } => _devices.time().saturating_sub(*start_time),
            #[cfg(slint_debug_performance)]
            Self::Stopped { elapsed } => *elapsed,
        }
    }

    pub fn stop(&mut self, _devices: &dyn Devices) {
        match self {
            #[cfg(slint_debug_performance)]
            Self::Running { .. } => {
                *self = Timer::Stopped { elapsed: self.elapsed(_devices) };
            }
            _ => {}
        }
    }

    pub fn start(&mut self, _devices: &dyn Devices) {
        match self {
            #[cfg(slint_debug_performance)]
            Self::Stopped { elapsed } => {
                *self = Self::Running { start_time: _devices.time().saturating_sub(*elapsed) }
            }
            _ => {}
        }
    }

    pub fn stop_profiling(self, _devices: &dyn Devices, _context: &'static str) {
        #[cfg(slint_debug_performance)]
        i_slint_core::debug_log!("{} took: {}ms", _context, self.elapsed(_devices).as_millis())
    }
}
