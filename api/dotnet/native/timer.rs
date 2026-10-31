// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::ffi::c_void;
use std::time::Duration;

use i_slint_core::timers::{Timer, TimerMode};

use crate::{guard, guard_bool, into_raw, release};

/// A .NET `Action`; dropping it releases `user_data` through `drop_user_data`.
pub(crate) struct ManagedAction {
    pub(crate) call: unsafe extern "C" fn(user_data: *mut c_void),
    pub(crate) user_data: *mut c_void,
    pub(crate) drop_user_data: unsafe extern "C" fn(user_data: *mut c_void),
}

impl ManagedAction {
    pub(crate) fn call(&self) {
        unsafe { (self.call)(self.user_data) }
    }
}

impl Drop for ManagedAction {
    fn drop(&mut self) {
        unsafe { (self.drop_user_data)(self.user_data) }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_timer_new() -> *mut Timer {
    guard(std::ptr::null_mut(), || Ok(into_raw(Timer::default())))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_timer_free(timer: *mut Timer) {
    unsafe { release(timer) }
}

/// `repeated` selects `TimerMode::Repeated` over `TimerMode::SingleShot`.
/// Takes ownership of `user_data`, also on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_timer_start(
    timer: &Timer,
    repeated: bool,
    interval_ms: u64,
    call: unsafe extern "C" fn(*mut c_void),
    user_data: *mut c_void,
    drop_user_data: unsafe extern "C" fn(*mut c_void),
) -> bool {
    let action = ManagedAction { call, user_data, drop_user_data };
    guard_bool(|| {
        let mode = if repeated { TimerMode::Repeated } else { TimerMode::SingleShot };
        timer.start(mode, Duration::from_millis(interval_ms), move || action.call());
        Ok(())
    })
}

/// Takes ownership of `user_data`, also on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_timer_single_shot(
    interval_ms: u64,
    call: unsafe extern "C" fn(*mut c_void),
    user_data: *mut c_void,
    drop_user_data: unsafe extern "C" fn(*mut c_void),
) -> bool {
    let action = ManagedAction { call, user_data, drop_user_data };
    guard_bool(|| {
        Timer::single_shot(Duration::from_millis(interval_ms), move || action.call());
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_timer_stop(timer: &Timer) -> bool {
    guard_bool(|| {
        timer.stop();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_timer_restart(timer: &Timer) -> bool {
    guard_bool(|| {
        timer.restart();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_timer_running(timer: &Timer) -> bool {
    guard(false, || Ok(timer.running()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_timer_set_interval(timer: &Timer, interval_ms: u64) -> bool {
    guard_bool(|| {
        timer.set_interval(Duration::from_millis(interval_ms));
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_timer_interval_ms(timer: &Timer) -> u64 {
    guard(0, || Ok(timer.interval().as_millis() as u64))
}
