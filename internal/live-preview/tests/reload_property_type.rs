// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(feature = "live-component")]

#[path = "live_reload/harness.rs"]
mod harness;

use i_slint_core::SharedString;
use i_slint_live_preview::live_component::Value;
use std::cell::Cell;
use std::rc::Rc;

const COMPATIBLE: &str = r#"
    export global Directory {
        in-out property <string> current: "initial";
        in-out property <string> description;
    }
    export component App inherits Window {
        out property <string> label: Directory.current;
        public function update-title() { Directory.current = "set in Slint"; }
    }
"#;

const INCOMPATIBLE: &str = r#"
    enum Location { home, work }
    export global Directory {
        in-out property <Location> current: Location.home;
        out property <string> description: "read from Slint";
    }
    export component App inherits Window {
        out property <Location> label: Directory.current;
    }
"#;

#[test]
fn rust_writes_survive_an_incompatible_reload() {
    let directory = harness::TestDir::new("property-type");
    let file = directory.write("app.slint", COMPATIBLE);
    let component = harness::start(&file);
    component.borrow_mut().set_error_hook(|error| eprintln!("Warning: Live preview: {error}"));
    for name in ["Directory.current", "Directory.description"] {
        component.borrow().set_rust_property(name, Value::String("before reload".into()));
    }

    let reloads = Rc::new(Cell::new(0));
    component.borrow_mut().set_post_reload_hook({
        let (component, file, reloads) = (Rc::downgrade(&component), file.clone(), reloads.clone());
        move |instance| {
            let component = component.upgrade().unwrap();
            let component = component.borrow();
            let read = |name| {
                component.get_rust_property(name, SharedString::try_from, SharedString::default)
            };
            reloads.set(reloads.get() + 1);
            if reloads.get() == 1 {
                assert_eq!(read("Directory.current"), "before reload");
                assert_eq!(read("Directory.description"), "read from Slint");
                assert_eq!(read("label"), "");
                component
                    .set_rust_property("Directory.current", Value::String("during reload".into()));
                assert_eq!(read("Directory.current"), "during reload");
                assert_eq!(
                    instance.get_global_property("Directory", "current").unwrap(),
                    Value::EnumerationValue("Location".into(), "home".into()),
                );
                std::fs::write(&file, COMPATIBLE).unwrap();
            } else {
                assert_eq!(
                    instance.get_property("label").unwrap(),
                    Value::String("during reload".into()),
                );
                instance.invoke("update-title", &[]).unwrap();
                assert_eq!(read("Directory.current"), "set in Slint");
                i_slint_core::api::quit_event_loop().unwrap();
            }
        }
    });

    harness::watchdog();
    std::fs::write(&file, INCOMPATIBLE).unwrap();
    slint_interpreter::run_event_loop().unwrap();
    assert_eq!(reloads.get(), 2);
}
