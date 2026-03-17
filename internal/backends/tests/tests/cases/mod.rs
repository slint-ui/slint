// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[test]
fn multiple_quit_event_loop_calls() {
    crate::init();

    slint::slint! {
        export component App inherits Window {
            Text { text: "Hello"; }
        }
    }

    let app = App::new().unwrap();

    // First run: quit immediately.
    slint::invoke_from_event_loop(|| {
        slint::quit_event_loop().unwrap();
        // duplicate quit_event_loop
        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    // Note: Exiting run will call `hide()`, which will trigger another `quit_event_loop()`
    app.run().unwrap();

    // quit_event_loop() while no event loop is active
    slint::quit_event_loop().unwrap();

    // Second run: the event loop must not exit immediately from the stale
    // quit above.
    let did_process_events = Arc::new(AtomicBool::new(false));
    let did_process_events_clone = did_process_events.clone();
    slint::invoke_from_event_loop(move || {
        // Note that we need to queue another invoke_from_event_loop here, as otherwise
        // the winit backend might not trigger the bug we want to test, because it
        // still processes the events in the same run() call before exiting.
        slint::invoke_from_event_loop(move || {
            did_process_events_clone.store(true, Ordering::Relaxed);
            slint::quit_event_loop().unwrap();
        })
        .unwrap();
    })
    .unwrap();
    app.run().unwrap();

    assert!(
        did_process_events.load(Ordering::Relaxed),
        "run() exited without processing events — stale quit event was consumed"
    );
}
