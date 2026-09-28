// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A plain `Timer` — the API that names no context — must land on the context that is
//! actually driving this thread, including when that context was created directly rather
//! than installed by `set_platform`.
//!
//! Getting this wrong is silent: the timer reports `running()`, but it sits in a list
//! nothing ticks, so it simply never fires.

use std::rc::Rc;

struct TestPlatform {
    start: std::time::Instant,
}

impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Err(slint::PlatformError::Other("this test needs no window".into()))
    }
    fn duration_since_start(&self) -> core::time::Duration {
        self.start.elapsed()
    }
}

#[test]
fn ambient_timer_lands_on_a_directly_created_context() {
    let ctx = i_slint_core::SlintContext::new(Box::new(TestPlatform {
        start: std::time::Instant::now(),
    }));

    let fired = Rc::new(std::cell::Cell::new(0));
    let fired_ = fired.clone();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(10), move || {
        fired_.set(fired_.get() + 1)
    });

    assert!(
        ctx.next_timer_timeout().is_some(),
        "the timer registered somewhere other than the context driving this thread"
    );

    std::thread::sleep(std::time::Duration::from_millis(50));
    ctx.update_timers_and_animations();
    assert!(fired.get() > 0, "the timer never fired despite its context being driven");
}
