// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Saving a project rewrites several files in a row, and an editor may write one
//! file more than once per save. The worker waits out that burst and compiles
//! the state it settles on, once, instead of compiling every intermediate state.

#![cfg(feature = "live-component")]

#[path = "live_reload/harness.rs"]
mod harness;

use i_slint_live_preview::live_component::{Compiler, Value};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Written back-to-back, so they all land within the worker's debounce.
const EDITS: std::ops::RangeInclusive<i32> = 2..=7;

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
fn a_burst_of_changes_is_compiled_once() {
    let dir = harness::TestDir::new("coalesced");
    dir.write("lib.slint", "export global Lib { out property <int> base: 100; }");
    let file = dir.write("app.slint", &app(1));

    // The loader counts compilations: it reports the import as unhandled, to be read from disk
    let compilations = Arc::new(AtomicUsize::new(0));
    let factory = {
        let compilations = compilations.clone();
        move || {
            let compilations = compilations.clone();
            let mut compiler = Compiler::default();
            compiler.set_file_loader(move |_| {
                compilations.fetch_add(1, Ordering::Relaxed);
                Box::pin(std::future::ready(None))
            });
            compiler
        }
    };

    let component = harness::start_with(factory, &file);
    let before = compilations.load(Ordering::Relaxed);
    assert_eq!(before, 1, "the first build should have loaded the import once");

    // A loaded machine can spread the burst over more than one reload, so wait for the last
    let settled = Value::Number(100. + *EDITS.end() as f64);
    component.borrow_mut().set_post_reload_hook(move |instance| {
        if instance.get_property("value").as_ref() == Ok(&settled) {
            i_slint_core::api::quit_event_loop().unwrap();
        }
    });
    harness::watchdog();
    for value in EDITS {
        std::fs::write(&file, app(value)).unwrap();
    }

    slint_interpreter::run_event_loop().unwrap();

    assert_eq!(
        component.borrow().get_property("value"),
        Value::Number(100. + *EDITS.end() as f64),
        "the last change should be the one that ends up applied"
    );
    let compilations = compilations.load(Ordering::Relaxed) - before;
    // A worker that compiled every request it was sent compiles eight to eleven times here
    assert!(
        compilations <= EDITS.count() / 2,
        "{compilations} compilations for {} changes: the burst was not coalesced",
        EDITS.count()
    );
}
