// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
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
    })
    .unwrap();
    app.run().unwrap();

    // Second run: the event loop must not exit immediately from the stale
    // quit above. A background thread sends the real quit after a delay.
    let did_process_events = Arc::new(AtomicBool::new(false));
    let did_process_events_clone = did_process_events.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        slint::invoke_from_event_loop(move || {
            did_process_events_clone.store(true, Ordering::Relaxed);
            slint::quit_event_loop().unwrap();
        })
        .unwrap();
    });
    app.run().unwrap();

    assert!(
        did_process_events.load(Ordering::Relaxed),
        "run() exited without processing events — stale quit event was consumed"
    );
}
