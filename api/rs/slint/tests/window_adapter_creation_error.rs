// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A window adapter creation failure must surface as an `Err` from `new()`. With a custom
//! font import, the font registration in `init()` is the first window adapter access and
//! used to panic instead. The attempt counter in the error message checks that each `new()`
//! makes exactly one creation attempt, i.e. that the error is reported by the first access
//! and not swallowed and re-triggered later.

use slint::platform::{Platform, PlatformError, WindowAdapter};
use std::cell::Cell;
use std::rc::Rc;

mod without_font {
    slint::slint! {
        export component WithoutFont inherits Window {
            Text {
                text: "Hello";
            }
        }
    }
}

mod with_font {
    slint::slint! {
        import "../../../examples/slide_puzzle/plaster-font/Plaster-Regular.ttf";

        export component WithFont inherits Window {
            Text {
                font-family: "Plaster";
                text: "Hello";
            }
        }
    }
}

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

#[test]
fn window_adapter_creation_error_is_returned() {
    slint::platform::set_platform(Box::new(FailingPlatform { attempts: Cell::new(0) })).unwrap();

    // Without fonts, the eager window adapter creation in `new()` reports the error.
    let Err(PlatformError::Other(message)) = without_font::WithoutFont::new() else {
        panic!("expected new() to fail with the platform error");
    };
    assert_eq!(message, "cannot create window adapter (attempt 1)");

    // With a custom font, the font registration in `init()` reports the error.
    let Err(PlatformError::Other(message)) = with_font::WithFont::new() else {
        panic!("expected new() to fail with the platform error");
    };
    assert_eq!(message, "cannot create window adapter (attempt 2)");
}
