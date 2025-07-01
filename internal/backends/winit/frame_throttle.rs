// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::{Rc, Weak};

use i_slint_core::timers::{Timer, TimerMode};

use crate::winitwindowadapter::WinitWindowAdapter;

pub fn create_frame_throttle(
    window_adapter: Weak<WinitWindowAdapter>,
    _is_wayland: bool,
) -> Box<dyn FrameThrottle> {
    if _is_wayland {
        WinitBasedFrameThrottle::new(window_adapter)
    } else {
        TimerBasedFrameThrottle::new(window_adapter)
    }
}

pub trait FrameThrottle {
    fn request_throttled_redraw(&self);
}

struct TimerBasedFrameThrottle {
    window_adapter: Weak<WinitWindowAdapter>,
    timer: Rc<Timer>,
}

impl TimerBasedFrameThrottle {
    fn new(window_adapter: Weak<WinitWindowAdapter>) -> Box<dyn FrameThrottle> {
        Box::new(Self { window_adapter, timer: Rc::new(Timer::default()) })
    }
}

impl FrameThrottle for TimerBasedFrameThrottle {
    fn request_throttled_redraw(&self) {
        if self.timer.running() {
            return;
        }
        let refresh_interval_millihertz = self
            .window_adapter
            .upgrade()
            .and_then(|adapter| adapter.winit_window())
            .and_then(|winit_window| winit_window.current_monitor())
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .unwrap_or(60000) as u64;
        let window_adapter = self.window_adapter.clone();
        let timer = Rc::downgrade(&self.timer);
        let interval =
            std::time::Duration::from_millis((1000 * 1000) / refresh_interval_millihertz);
        self.timer.start(TimerMode::Repeated, interval, move || {
            redraw_now(&window_adapter);

            let Some(timer) = timer.upgrade() else { return };
            let Some(window_adapter) = window_adapter.upgrade() else { return };

            let keep_running = window_adapter.pending_redraw();

            if timer.running() {
                if !keep_running {
                    timer.stop();
                }
            } else {
                if keep_running {
                    timer.restart();
                }
            }
        });
    }
}

fn redraw_now(window_adapter: &Weak<WinitWindowAdapter>) {
    let Some(winit_window) = window_adapter.upgrade().and_then(|adapter| adapter.winit_window())
    else {
        return;
    };
    winit_window.request_redraw();
}

struct WinitBasedFrameThrottle {
    window_adapter: Weak<WinitWindowAdapter>,
}

impl WinitBasedFrameThrottle {
    fn new(window_adapter: Weak<WinitWindowAdapter>) -> Box<dyn FrameThrottle> {
        Box::new(Self { window_adapter })
    }
}

impl FrameThrottle for WinitBasedFrameThrottle {
    fn request_throttled_redraw(&self) {
        redraw_now(&self.window_adapter)
    }
}
