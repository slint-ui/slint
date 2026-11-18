// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore vblank
use std::rc::{Rc, Weak};

use i_slint_core::timers::{Timer, TimerMode};

#[cfg(target_vendor = "apple")]
mod apple_display_link;

use crate::winitwindowadapter::{WindowVisibility, WinitWindowAdapter};

pub fn create_frame_throttle(
    window_adapter: Weak<WinitWindowAdapter>,
    _winit_window: &winit::window::Window,
    _is_wayland: bool,
) -> Box<dyn FrameThrottle> {
    // Winit already schedules web redraws through requestAnimationFrame.
    if cfg!(target_arch = "wasm32") || _is_wayland {
        WinitBasedFrameThrottle::create()
    } else {
        #[cfg(target_vendor = "apple")]
        if let Some(throttle) =
            apple_display_link::try_create(window_adapter.clone(), _winit_window)
        {
            return throttle;
        }
        TimerBasedFrameThrottle::create(window_adapter)
    }
}

pub trait FrameThrottle {
    fn request_throttled_redraw(&self, winit_window: &winit::window::Window);
}

struct TimerBasedFrameThrottle {
    window_adapter: Weak<WinitWindowAdapter>,
    timer: Rc<Timer>,
}

impl TimerBasedFrameThrottle {
    fn create(window_adapter: Weak<WinitWindowAdapter>) -> Box<dyn FrameThrottle> {
        Box::new(Self { window_adapter, timer: Rc::new(Timer::default()) })
    }
}

impl FrameThrottle for TimerBasedFrameThrottle {
    fn request_throttled_redraw(&self, winit_window: &winit::window::Window) {
        if self.timer.running() {
            return;
        }
        winit_window.request_redraw();
        let refresh_interval_millihertz = winit_window
            .current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .filter(|&millihertz| millihertz != 0)
            .unwrap_or(60000);
        let window_adapter = self.window_adapter.clone();
        let timer = Rc::downgrade(&self.timer);
        let interval = refresh_interval(refresh_interval_millihertz);
        self.timer.start(TimerMode::Repeated, next_frame(interval), move || {
            let Some(adapter) = window_adapter.upgrade() else { return };
            if adapter.pending_redraw()
                && adapter.visibility() != WindowVisibility::Hidden
                && !adapter.is_minimized_on_screen()
                && let Some(winit_window) = adapter.winit_window()
            {
                winit_window.request_redraw();
                #[cfg(target_os = "windows")]
                if let Some(timer) = timer.upgrade() {
                    timer.set_interval(next_frame(interval));
                }
            } else {
                // Showing a hidden window draws it.
                // Windows sends no WM_PAINT to a minimized window, and repaints it on restore.
                adapter.cancel_pending_redraw();
                if let Some(timer) = timer.upgrade() {
                    timer.stop();
                }
            }
        });
    }
}

fn refresh_interval(millihertz: u32) -> std::time::Duration {
    std::time::Duration::from_nanos(1_000_000_000_000 / u64::from(millihertz))
}

fn next_frame(interval: std::time::Duration) -> std::time::Duration {
    #[cfg(target_os = "windows")]
    if let Some(delay) = compositor_clock::until_next_vblank(interval) {
        return delay;
    }
    interval
}

/// The time from `now` to the first tick after it, of a clock that ticked at `last` and every
/// `period` since, all counted at `frequency` per second.
///
/// `None` when `period` is more than a tenth away from `interval`, as for a window on a monitor
/// with another refresh rate than the one DWM follows.
#[cfg(any(target_os = "windows", test))]
fn until_next_tick(
    now: i64,
    last: i64,
    period: i64,
    frequency: i64,
    interval: std::time::Duration,
) -> Option<std::time::Duration> {
    if frequency <= 0 || period <= 0 {
        return None;
    }
    let duration = |count: i64| {
        let nanos = count.max(0) as u128 * 1_000_000_000 / frequency as u128;
        std::time::Duration::from_nanos(nanos as u64)
    };
    let ratio = duration(period).as_secs_f64() / interval.as_secs_f64();
    if !(0.9..1.1).contains(&ratio) {
        return None;
    }
    Some(duration(period - (now - last).rem_euclid(period)))
}

#[cfg(target_os = "windows")]
mod compositor_clock {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmGetCompositionTimingInfo};
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

    pub fn until_next_vblank(interval: std::time::Duration) -> Option<std::time::Duration> {
        let mut info = DWM_TIMING_INFO {
            cbSize: core::mem::size_of::<DWM_TIMING_INFO>() as u32,
            ..Default::default()
        };
        // Since Windows 8.1 the window must be null.
        unsafe { DwmGetCompositionTimingInfo(HWND::default(), &mut info) }.ok()?;
        let (mut now, mut frequency) = (0i64, 0i64);
        unsafe { QueryPerformanceCounter(&mut now) }.ok()?;
        unsafe { QueryPerformanceFrequency(&mut frequency) }.ok()?;
        super::until_next_tick(
            now,
            i64::try_from(info.qpcVBlank).ok()?,
            i64::try_from(info.qpcRefreshPeriod).ok()?,
            frequency,
            interval,
        )
    }
}

struct WinitBasedFrameThrottle;

impl WinitBasedFrameThrottle {
    fn create() -> Box<dyn FrameThrottle> {
        Box::new(Self)
    }
}

impl FrameThrottle for WinitBasedFrameThrottle {
    fn request_throttled_redraw(&self, winit_window: &winit::window::Window) {
        winit_window.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn refresh_interval_keeps_fractions_of_a_millisecond() {
        assert_eq!(refresh_interval(60_000), Duration::from_nanos(16_666_666));
        assert_eq!(refresh_interval(59_940), Duration::from_nanos(16_683_350));
        assert_eq!(refresh_interval(144_000), Duration::from_nanos(6_944_444));
    }

    #[test]
    fn next_tick_follows_the_last_vblank() {
        let ten_mhz = 10_000_000;
        let refresh_at_60_hz = 166_667;
        let last = 1_000_000;
        let at_60_hz = refresh_interval(60_000);
        let next = |now| until_next_tick(now, last, refresh_at_60_hz, ten_mhz, at_60_hz);
        assert_eq!(next(last), Some(Duration::from_nanos(16_666_700)));
        assert_eq!(next(last + 50), Some(Duration::from_nanos(16_661_700)));
        assert_eq!(next(last + 10 * refresh_at_60_hz + 7), Some(Duration::from_nanos(16_666_000)));
        assert_eq!(next(last - 7), Some(Duration::from_nanos(700)));
        let at_144_hz = refresh_interval(144_000);
        assert_eq!(until_next_tick(last, last, refresh_at_60_hz, ten_mhz, at_144_hz), None);
        assert_eq!(until_next_tick(last, last, 0, ten_mhz, at_60_hz), None);
        assert_eq!(until_next_tick(last, last, refresh_at_60_hz, 0, at_60_hz), None);
    }
}
