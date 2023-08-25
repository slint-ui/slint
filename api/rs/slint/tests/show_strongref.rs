// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use ::slint::slint;

// Sorry, can't test with rust test harness and multiple threads.
#[cfg(not(any(target_arch = "wasm32", target_os = "macos", target_os = "ios")))]
#[test]
fn show_maintains_strong_reference() {
    slint::platform::set_platform(Box::new(i_slint_backend_winit::Backend::new())).unwrap();

    slint!(export component TestWindow inherits Window {
        callback root-clicked();
        TouchArea {
            clicked => { root.root-clicked(); }
        }
    });

    let window = TestWindow::new().unwrap();

    let window_weak = window.as_weak();
    let window_weak_2 = window_weak.clone();

    slint::Timer::single_shot(std::time::Duration::from_millis(20), move || {
        window_weak_2.upgrade().unwrap().hide().unwrap();
        slint::quit_event_loop().unwrap();
    });

    window.show().unwrap();
    drop(window);
    slint::run_event_loop().unwrap();

    assert!(window_weak.upgrade().is_none());
}
