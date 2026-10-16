// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The point of compiling a reload on a worker thread is that the event loop
//! keeps running while it happens: a compilation that needs the loop to run
//! something must still finish.

#![cfg(feature = "live-component")]

#[path = "live_reload/harness.rs"]
mod harness;

use i_slint_live_preview::live_component::{Compiler, Value};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn app(value: i32) -> String {
    format!(
        r#"
        import {{ Lib }} from "lib.slint";
        export component App inherits Window {{
            out property <int> value: Lib.base + {value};
        }}"#
    )
}

#[test]
fn compiling_a_reload_does_not_block_the_event_loop() {
    let dir = harness::TestDir::new("threaded");
    dir.write("lib.slint", "export global Lib { out property <int> base: 100; }");
    let file = dir.write("app.slint", &app(1));

    // The first build runs before the event loop does, so only reloads wait on it
    let armed = Arc::new(AtomicBool::new(false));
    let factory = {
        let armed = armed.clone();
        move || {
            let mut compiler = Compiler::default();
            compiler.set_file_loader(move |_| {
                let mut result = None;
                if armed.load(Ordering::Relaxed) {
                    let (ran, ran_rx) = std::sync::mpsc::channel();
                    i_slint_core::api::invoke_from_event_loop(move || {
                        let _ = ran.send(());
                    })
                    .unwrap();
                    if ran_rx.recv_timeout(harness::RELOAD_TIMEOUT).is_err() {
                        result = Some(Err(std::io::Error::other(
                            "the event loop didn't run while the reload compiled",
                        )));
                    }
                }
                Box::pin(std::future::ready(result))
            });
            compiler
        }
    };

    let component = harness::start_with(factory, &file);
    assert_eq!(component.borrow().get_property("value"), Value::Number(101.));

    armed.store(true, Ordering::Relaxed);
    component.borrow_mut().set_post_reload_hook(|_| {
        i_slint_core::api::quit_event_loop().unwrap();
    });
    harness::watchdog();

    std::fs::write(&file, app(2)).unwrap();
    slint_interpreter::run_event_loop().unwrap();

    assert_eq!(
        component.borrow().get_property("value"),
        Value::Number(102.),
        "the reload should have compiled while the event loop kept running"
    );
}
