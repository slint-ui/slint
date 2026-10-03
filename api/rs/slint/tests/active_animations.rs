// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `Window::has_active_animations()` only reports the animations of its own window (#6259).

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use slint::{ComponentHandle, PhysicalSize};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

thread_local! {
    static CLOCK: Cell<Duration> = const { Cell::new(Duration::ZERO) };
    static WINDOWS: RefCell<Vec<Rc<MinimalSoftwareWindow>>> = const { RefCell::new(Vec::new()) };
}

struct TestPlatform;

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        window.set_size(PhysicalSize::new(100, 100));
        WINDOWS.with(|w| w.borrow_mut().push(window.clone()));
        Ok(window)
    }

    fn duration_since_start(&self) -> Duration {
        CLOCK.with(Cell::get)
    }
}

slint::slint! {
    export component Animated inherits Window {
        in property <bool> animate: true;
        out property <duration> tick: animation-tick();
        Rectangle {
            x: animate ? animation-tick() / 1ms * 1px : 0;
            width: 10px;
            height: 10px;
            background: red;
        }
    }

    export component Static inherits Window {
        Rectangle {
            width: 10px;
            height: 10px;
            background: blue;
        }
    }
}

fn next_frame() {
    CLOCK.with(|c| c.set(c.get() + Duration::from_millis(16)));
    slint::platform::update_timers_and_animations();
    for window in WINDOWS.with(|w| w.borrow().clone()) {
        window.draw_if_needed(|renderer| {
            let mut buffer = vec![slint::Rgb8Pixel::default(); 100 * 100];
            renderer.render(&mut buffer, 100);
        });
    }
}

#[test]
fn has_active_animations_is_per_window() {
    slint::platform::set_platform(Box::new(TestPlatform)).unwrap();

    let animated = Animated::new().unwrap();
    let stationary = Static::new().unwrap();
    animated.show().unwrap();
    stationary.show().unwrap();

    next_frame();
    assert!(animated.window().has_active_animations());
    assert!(!stationary.window().has_active_animations());

    // An animated property that no window draws doesn't count.
    animated.set_animate(false);
    next_frame();
    animated.get_tick();
    assert!(!animated.window().has_active_animations());
    assert!(!stationary.window().has_active_animations());
}
