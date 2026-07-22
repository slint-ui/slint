// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::Devices;

#[cfg(slint_debug_performance)]
pub enum Timer {
    Running { start_time: core::time::Duration },
    Stopped { elapsed: core::time::Duration },
}

#[cfg(not(slint_debug_performance))]
pub struct Timer {}

impl Timer {
    pub fn new(_devices: &dyn Devices) -> Self {
        #[cfg(slint_debug_performance)]
        return Self::Running { start_time: _devices.time() };
        #[cfg(not(slint_debug_performance))]
        return Self {};
    }
    pub fn new_stopped() -> Self {
        #[cfg(slint_debug_performance)]
        return Self::Stopped { elapsed: core::time::Duration::new(0, 0) };
        #[cfg(not(slint_debug_performance))]
        return Self {};
    }

    #[cfg(slint_debug_performance)]
    pub fn elapsed(&self, _devices: &dyn Devices) -> core::time::Duration {
        match self {
            Self::Running { start_time } => _devices.time().saturating_sub(*start_time),
            Self::Stopped { elapsed } => *elapsed,
        }
    }

    pub fn stop(&mut self, _devices: &dyn Devices) {
        #[cfg(slint_debug_performance)]
        match self {
            Self::Running { .. } => {
                *self = Timer::Stopped { elapsed: self.elapsed(_devices) };
            }
            _ => {}
        }
    }

    pub fn start(&mut self, _devices: &dyn Devices) {
        #[cfg(slint_debug_performance)]
        match self {
            Self::Stopped { elapsed } => {
                *self = Self::Running { start_time: _devices.time().saturating_sub(*elapsed) }
            }
            _ => {}
        }
    }

    pub fn stop_profiling(&mut self, _devices: &dyn Devices, _context: &'static str) {
        #[cfg(slint_debug_performance)]
        i_slint_core::debug_log!("{} took: {}ms", _context, self.elapsed(_devices).as_millis())
    }
}
