// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! There is no synchronous fallback left: a reload is applied only when the
//! worker produced a result that compiles. Check what happens around a save that
//! doesn't compile -- the running instance stays live, and the next good save
//! still reloads.

#![cfg(feature = "live-component")]

#[path = "live_reload/harness.rs"]
mod harness;

use i_slint_live_preview::live_component::Value;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

/// Long enough for the worker to have compiled the broken source and for the
/// event loop to have applied a reload, had there been one to apply.
const GRACE: Duration = Duration::from_secs(2);

fn app(value: i32) -> String {
    format!(
        r#"
        export component App inherits Window {{
            in-out property <int> counter;
            out property <int> value: {value};
            public function bump() {{ root.counter += 1; }}
        }}"#
    )
}

#[test]
fn a_reload_that_does_not_compile_keeps_the_running_instance() {
    let dir = harness::TestDir::new("after-error");
    let file = dir.write("app.slint", &app(1));

    let component = harness::start(&file);

    let reloads = Rc::new(Cell::new(0));
    component.borrow_mut().set_post_reload_hook({
        let reloads = reloads.clone();
        move |_| {
            reloads.set(reloads.get() + 1);
            i_slint_core::api::quit_event_loop().unwrap();
        }
    });

    std::fs::write(&file, "export component App inherits { oops").unwrap();

    let checked = Rc::new(Cell::new(false));
    harness::watchdog();
    harness::after(GRACE, {
        let (component, reloads, checked) = (component.clone(), reloads.clone(), checked.clone());
        move || {
            assert_eq!(reloads.get(), 0, "a source that doesn't compile was applied");
            let borrowed = component.borrow();
            assert_eq!(borrowed.get_property("value"), Value::Number(1.));
            borrowed.invoke("bump", &[]);
            assert_eq!(
                borrowed.get_property("counter"),
                Value::Number(1.),
                "the instance from before the failed reload should still be live"
            );
            checked.set(true);
            std::fs::write(&file, app(2)).unwrap();
        }
    });

    slint_interpreter::run_event_loop().unwrap();

    assert!(checked.get(), "the loop quit before the failed reload was examined");
    assert_eq!(reloads.get(), 1);
    assert_eq!(
        component.borrow().get_property("value"),
        Value::Number(2.),
        "a good save after a failed one should still reload"
    );
}
