// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A component is rarely one file, and which files it reads changes as it is
//! edited. Every compilation names them, and the watcher follows: an import the
//! worker reported reloads the component when it is edited, including one that
//! only a reload introduced.

#![cfg(feature = "live-component")]

#[path = "live_reload/harness.rs"]
mod harness;

use i_slint_live_preview::live_component::Value;
use std::cell::Cell;
use std::rc::Rc;

/// Reads `lib.slint` only.
const APP: &str = r#"
    import { Lib } from "lib.slint";
    export component App inherits Window {
        out property <int> value: Lib.base;
    }"#;

/// Reads `extra.slint` as well, which the first version never mentioned.
const APP_WITH_EXTRA: &str = r#"
    import { Lib } from "lib.slint";
    import { Extra } from "extra.slint";
    export component App inherits Window {
        out property <int> value: Lib.base + Extra.bonus;
    }"#;

fn extra(bonus: i32) -> String {
    format!("export global Extra {{ out property <int> bonus: {bonus}; }}")
}

#[test]
fn editing_an_imported_file_reloads() {
    let dir = harness::TestDir::new("imported");
    dir.write("lib.slint", "export global Lib { out property <int> base: 100; }");
    dir.write("extra.slint", &extra(0));
    let file = dir.write("app.slint", APP);

    let component = harness::start(&file);
    assert_eq!(component.borrow().get_property("value"), Value::Number(100.));

    // The first reload is what puts extra.slint under watch; the second answers editing it
    let reloads = Rc::new(Cell::new(0));
    component.borrow_mut().set_post_reload_hook({
        let (dir_path, reloads) = (dir.path().to_path_buf(), reloads.clone());
        move |_| {
            reloads.set(reloads.get() + 1);
            match reloads.get() {
                1 => std::fs::write(dir_path.join("extra.slint"), extra(7)).unwrap(),
                _ => i_slint_core::api::quit_event_loop().unwrap(),
            }
        }
    });
    harness::watchdog();
    std::fs::write(&file, APP_WITH_EXTRA).unwrap();

    slint_interpreter::run_event_loop().unwrap();

    assert_eq!(
        component.borrow().get_property("value"),
        Value::Number(107.),
        "editing an import the last reload introduced should have reloaded the component"
    );
}
