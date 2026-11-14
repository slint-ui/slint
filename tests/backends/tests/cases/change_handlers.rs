// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

/// #14792: the window isn't shown, so no repaint runs the change handlers.
#[satchel::test]
fn change_handlers_run_after_invoke_from_event_loop() {
    slint::slint! {
        #[edition = "1.19"]
        export component App inherits Window {
            in property <int> input;
            out property <int> observed;
            changed input => { observed = input; }
        }
    }

    let app = App::new().unwrap();
    let weak = app.as_weak();
    let seen = Arc::new(AtomicI32::new(-1));
    let seen_clone = seen.clone();
    slint::invoke_from_event_loop(move || {
        weak.unwrap().set_input(1);
        slint::invoke_from_event_loop(move || {
            seen_clone.store(weak.unwrap().get_observed(), Ordering::Relaxed);
            slint::quit_event_loop().unwrap();
        })
        .unwrap();
    })
    .unwrap();
    slint::run_event_loop_until_quit().unwrap();

    assert_eq!(seen.load(Ordering::Relaxed), 1);
}
