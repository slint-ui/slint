// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A window adapter creation failure must surface as an `Err` from `create()`. With a custom
//! font import, the font registration during instantiation is the first window adapter access
//! and used to panic instead. The attempt counter in the error message documents the flow:
//! `instantiate()` has no error channel, so the registration logs and skips its failed attempt,
//! and the eager creation in `create()` reports the error of the next attempt. The test also
//! checks that the skipped registration is logged rather than swallowed silently.

use i_slint_core::platform::{Platform, PlatformError, WindowAdapter, set_platform};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct FailingPlatform {
    attempts: Cell<u32>,
}

impl Platform for FailingPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        self.attempts.set(self.attempts.get() + 1);
        Err(PlatformError::Other(format!(
            "cannot create window adapter (attempt {})",
            self.attempts.get()
        )))
    }
}

fn compile(code: &str) -> slint_interpreter::ComponentDefinition {
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/test.slint");
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), path.into()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    result.component("TestCase").unwrap()
}

#[test]
fn window_adapter_creation_error_is_returned() {
    set_platform(Box::new(FailingPlatform { attempts: Cell::new(0) })).unwrap();

    // Without fonts, the eager window adapter creation in `create()` reports the error.
    let definition = compile(
        r#"
        export component TestCase inherits Window {
            Text {
                text: "Hello";
            }
        }
    "#,
    );
    let Err(PlatformError::Other(message)) = definition.create() else {
        panic!("expected create() to fail with the platform error");
    };
    assert_eq!(message, "cannot create window adapter (attempt 1)");

    // With a custom font, the registration during instantiation logs and skips attempt 2,
    // and the eager creation in `create()` reports attempt 3.
    let logs = Rc::new(RefCell::new(Vec::<String>::new()));
    let logs_handler = logs.clone();
    i_slint_backend_selector::with_global_context(|ctx| {
        ctx.set_log_message_handler(Some(Box::new(move |message| {
            logs_handler.borrow_mut().push(message.message_arguments().to_string());
        })))
    })
    .unwrap();

    let definition = compile(
        r#"
        import "../../../examples/slide_puzzle/plaster-font/Plaster-Regular.ttf";

        export component TestCase inherits Window {
            Text {
                font-family: "Plaster";
                text: "Hello";
            }
        }
    "#,
    );
    let Err(PlatformError::Other(message)) = definition.create() else {
        panic!("expected create() to fail with the platform error");
    };
    assert_eq!(message, "cannot create window adapter (attempt 3)");

    // The window adapter failure that skipped the registration (attempt 2) is logged rather
    // than swallowed silently, and the log is about creating the window, not the font.
    let logs = logs.borrow();
    assert!(
        logs.iter().any(|l| l == "cannot create window adapter (attempt 2)"),
        "expected a log about the window adapter creation failure, got {logs:?}"
    );
}
