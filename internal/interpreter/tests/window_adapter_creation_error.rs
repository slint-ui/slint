// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A window adapter creation failure must surface as an `Err` from `create()`, not a panic.
//! The platform is asked once per instance: the first failure is cached and reported by
//! every later access, like generated code where the error propagates out of `new()` at
//! the first access. With a custom font import, that first access is the registration
//! during instantiation, which logs the failure and skips instead of swallowing it.
//! The attempt counter in the message asserts both properties: which access failed first,
//! and that no path asked the platform again.

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

    // Without fonts, instantiation makes the first (and only) creation attempt and
    // `create()` reports its error.
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

    // With a custom font, the registration during instantiation is the first access:
    // its failure is logged, skipped, and reported by `create()`.
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
    assert_eq!(message, "cannot create window adapter (attempt 2)");

    // The failure that skipped the font registration is logged, and it is the same
    // first error that create() reports.
    let logs = logs.borrow();
    assert!(
        logs.iter().any(|l| l == "cannot create window adapter (attempt 2)"),
        "expected the skipped font registration to log the adapter failure, got {logs:?}"
    );
}
