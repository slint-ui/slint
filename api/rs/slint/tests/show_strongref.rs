// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use ::slint::slint;

#[test]
fn show_maintains_strong_reference() {
    i_slint_backend_testing::init_integration_test_with_mock_time();

    slint!(export component TestWindow inherits Window {
        callback root-clicked();
        TouchArea {
            clicked => { root.root-clicked(); }
        }
    });

    let window = TestWindow::new().unwrap();

    let window_weak = window.as_weak();
    let window_weak_2 = window_weak.clone();

    slint::invoke_from_event_loop(move || {
        window_weak_2.upgrade().unwrap().hide().unwrap();
        slint::quit_event_loop().unwrap();
    })
    .unwrap();

    window.show().unwrap();
    drop(window);
    slint::run_event_loop().unwrap();

    assert!(window_weak.upgrade().is_none());
}
