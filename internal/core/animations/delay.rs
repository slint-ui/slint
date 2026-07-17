// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::animations::Animation;
use alloc::boxed::Box;

/// Delays before starting the next animation
#[allow(dead_code)]
pub struct DelayAnimation {
    delay_ms: u64,
    start_time: crate::animations::Instant,
    running: bool,
    on_finished: Option<Box<dyn FnMut()>>,
}

#[allow(dead_code)]
impl DelayAnimation {
    /// Creates a new delay of `delay_ms` milliseconds, starting now.
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            start_time: crate::animations::current_tick(),
            running: true,
            on_finished: None,
        }
    }

    /// Returns true once `delay_ms` has elapsed since the delay started (or restarted).
    pub fn is_finished(&self) -> bool {
        let elapsed = crate::animations::current_tick().duration_since(self.start_time);
        elapsed.as_millis() as u64 >= self.delay_ms
    }
}

impl Animation for DelayAnimation {
    fn start(&mut self) {
        self.running = true;
    }

    fn stop(&mut self) {
        self.running = false;
    }

    fn restart(&mut self) {
        self.start_time = crate::animations::current_tick();
        self.running = true;
    }

    fn is_running(&self) -> bool {
        self.running && !self.is_finished()
    }

    fn update(&mut self) -> bool {
        let running = self.is_running();
        if running {
            // Nothing to compute while delaying, but the frame loop must keep
            // updating us so `is_finished()` gets observed once the delay elapses.
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
        } else if let Some(mut on_finished) = self.on_finished.take() {
            on_finished();
        }
        running
    }

    fn set_on_finished(&mut self, on_finished: Box<dyn FnMut()>) {
        self.on_finished = Some(on_finished);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_animation_lifecycle() {
        let start_time = crate::animations::current_tick();
        let mut delay = DelayAnimation::new(100);
        assert!(delay.is_running());
        assert!(!delay.is_finished());

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(50))
        });
        assert!(delay.update());
        assert!(delay.is_running());

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(150))
        });
        assert!(!delay.update());
        assert!(delay.is_finished());
        assert!(!delay.is_running());

        // restart() resets the clock, so the delay is running again.
        delay.restart();
        assert!(delay.is_running());
        assert!(!delay.is_finished());

        delay.stop();
        assert!(!delay.is_running());
    }
}
