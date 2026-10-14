// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The point of compiling a reload on a worker thread is that the event loop
//! keeps running while it happens. Check both halves: the new source is applied,
//! and the loop ran something posted to it while the compilation was waiting on it.

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

    // Once armed, loading the import holds the compilation until the event loop has
    // run a closure the loader posted to it. A compilation on the event-loop thread
    // can't get that closure run, so it waits out the timeout instead.
    let armed = Arc::new(AtomicBool::new(false));
    let loop_ran_during_compile = Arc::new(AtomicBool::new(false));
    let factory = {
        let armed = armed.clone();
        let loop_ran_during_compile = loop_ran_during_compile.clone();
        move || {
            let armed = armed.clone();
            let loop_ran_during_compile = loop_ran_during_compile.clone();
            let mut compiler = Compiler::default();
            compiler.set_file_loader(move |_| {
                if armed.load(Ordering::Relaxed) {
                    let (ran, ran_rx) = std::sync::mpsc::channel();
                    i_slint_core::api::invoke_from_event_loop(move || {
                        let _ = ran.send(());
                    })
                    .unwrap();
                    if ran_rx.recv_timeout(harness::RELOAD_TIMEOUT).is_ok() {
                        loop_ran_during_compile.store(true, Ordering::Relaxed);
                    }
                }
                Box::pin(std::future::ready(None))
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
        "the worker's result should have been applied to the instance"
    );
    assert!(
        loop_ran_during_compile.load(Ordering::Relaxed),
        "the event loop did not run while the reload compiled, \
         so the compilation was not running on the worker thread"
    );
}
